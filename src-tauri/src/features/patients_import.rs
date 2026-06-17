use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::CreatePatientInput;
use crate::errors::AppError;
use crate::features;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvRow {
    pub line: usize,
    pub full_name: String,
    pub chart_number: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub health_history: Option<String>,
    pub medications_in_use: Option<String>,
    pub emergency_phone: Option<String>,
    pub admin_notes: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub total_rows: usize,
    pub valid_rows: usize,
    pub error_rows: usize,
    pub rows: Vec<CsvRow>,
}

pub fn parse_csv_bytes(data: &[u8]) -> Result<ImportPreview, AppError> {
    let content = std::str::from_utf8(data)
        .map_err(|_| AppError::bad_request("Arquivo não é um CSV válido (UTF-8)."))?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let headers = reader.headers()
        .map_err(|e| AppError::bad_request(format!("Erro ao ler cabeçalho CSV: {}", e)))?
        .clone();

    let header_strs: Vec<String> = headers.iter().map(|h| h.trim().to_lowercase()).collect();

    let mut rows = Vec::new();

    for (i, result) in reader.records().enumerate() {
        let line = i + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                rows.push(CsvRow {
                    line,
                    full_name: String::new(),
                    chart_number: None,
                    phone: None,
                    email: None,
                    birth_date: None,
                    health_history: None,
                    medications_in_use: None,
                    emergency_phone: None,
                    admin_notes: None,
                    errors: vec![format!("Erro na linha {}: {}", line, e)],
                });
                continue;
            }
        };

        let mut row_errors = Vec::new();

        let get_field = |col: &str| -> Option<String> {
            header_strs.iter().position(|h| h.as_str() == col).and_then(|pos| {
                let val = record.get(pos)?.trim().to_string();
                if val.is_empty() { None } else { Some(val) }
            })
        };

        let full_name = get_field("full_name").or_else(|| get_field("nome")).or_else(|| get_field("name"));
        let full_name = match full_name {
            Some(n) if n.len() >= 2 => n,
            Some(_) => {
                row_errors.push("Nome deve ter no mínimo 2 caracteres.".into());
                String::new()
            }
            None => {
                row_errors.push("Nome é obrigatório (coluna 'full_name' ou 'nome').".into());
                String::new()
            }
        };

        rows.push(CsvRow {
            line,
            full_name,
            chart_number: get_field("chart_number").or_else(|| get_field("prontuario")).or_else(|| get_field("chart")),
            phone: get_field("phone").or_else(|| get_field("telefone")),
            email: get_field("email"),
            birth_date: get_field("birth_date").or_else(|| get_field("birthdate")).or_else(|| get_field("nascimento")),
            health_history: get_field("health_history").or_else(|| get_field("health")).or_else(|| get_field("historico")),
            medications_in_use: get_field("medications_in_use").or_else(|| get_field("medications")).or_else(|| get_field("medicamentos")),
            emergency_phone: get_field("emergency_phone").or_else(|| get_field("emergency")).or_else(|| get_field("emergencia")),
            admin_notes: get_field("admin_notes").or_else(|| get_field("notes")).or_else(|| get_field("observacoes")),
            errors: row_errors,
        });
    }

    let total_rows = rows.len();
    let valid_rows = rows.iter().filter(|r| r.errors.is_empty()).count();
    let error_rows = total_rows - valid_rows;

    Ok(ImportPreview {
        total_rows,
        valid_rows,
        error_rows,
        rows,
    })
}

pub async fn commit_import(
    db: &SqlitePool,
    user_id: &str,
    rows: &[CsvRow],
) -> Result<i64, AppError> {
    let mut imported = 0i64;

    for row in rows {
        if !row.errors.is_empty() {
            continue;
        }

        let input = CreatePatientInput {
            full_name: row.full_name.clone(),
            chart_number: row.chart_number.clone(),
            phone: row.phone.clone(),
            email: row.email.clone(),
            birth_date: row.birth_date.clone(),
            health_history: row.health_history.clone(),
            medications_in_use: row.medications_in_use.clone(),
            emergency_phone: row.emergency_phone.clone(),
            admin_notes: row.admin_notes.clone(),
        };

        features::patients::create_patient(db, user_id, &input).await?;
        imported += 1;
    }

    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_csv() {
        let csv = "full_name,phone,email\nJoão Silva,11999999999,joao@test.com\nMaria Souza,11988888888,maria@test.com\n";
        let result = parse_csv_bytes(csv.as_bytes()).unwrap();
        assert_eq!(result.total_rows, 2);
        assert_eq!(result.valid_rows, 2);
        assert_eq!(result.rows[0].full_name, "João Silva");
        assert_eq!(result.rows[1].full_name, "Maria Souza");
    }

    #[test]
    fn test_parse_csv_with_portuguese_headers() {
        let csv = "nome,telefone,email\nPedro Alves,11977777777,pedro@test.com\n";
        let result = parse_csv_bytes(csv.as_bytes()).unwrap();
        assert_eq!(result.total_rows, 1);
        assert_eq!(result.valid_rows, 1);
        assert_eq!(result.rows[0].full_name, "Pedro Alves");
    }

    #[test]
    fn test_parse_csv_missing_name() {
        let csv = "phone\n11999999999\n";
        let result = parse_csv_bytes(csv.as_bytes()).unwrap();
        assert_eq!(result.total_rows, 1);
        assert_eq!(result.valid_rows, 0);
        assert!(!result.rows[0].errors.is_empty());
    }

    #[test]
    fn test_parse_csv_invalid_utf8() {
        let bytes = vec![0xFF, 0xFE, 0x00];
        let result = parse_csv_bytes(&bytes);
        assert!(result.is_err());
    }
}
