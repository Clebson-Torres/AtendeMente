use chrono::NaiveDate;

/// Formats cents (e.g. 123456) to BRL currency string (e.g. "R$ 1.234,56")
pub fn format_currency_brl(cents: i64) -> String {
    let value = cents as f64 / 100.0;
    let formatted = format!("{:.2}", value);
    // Replace decimal point with comma and add thousands separator
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "00" };

    // Add thousands separator
    let mut with_sep = String::new();
    for (i, c) in integer_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            with_sep.push('.');
        }
        with_sep.push(c);
    }
    let integer_formatted: String = with_sep.chars().rev().collect();

    format!("R$ {},{}", integer_formatted, decimal_part)
}

/// Parses a BRL currency string (e.g. "R$ 1.234,56") to cents
pub fn parse_brl_to_cents(value: &str) -> Option<i64> {
    if value.is_empty() {
        return Some(0);
    }

    let sanitized: String = value
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();

    if sanitized.is_empty() {
        return Some(0);
    }

    let last_comma = sanitized.rfind(',');
    let last_dot = sanitized.rfind('.');

    let normalized = match (last_comma, last_dot) {
        (Some(comma_pos), Some(dot_pos)) => {
            if comma_pos > dot_pos {
                // Brazilian format: 1.234,56
                sanitized.replace('.', "").replace(',', ".")
            } else {
                // US format: 1,234.56
                sanitized.replace(',', "")
            }
        }
        (Some(_), None) => sanitized.replace(',', "."),
        (None, Some(dot_pos)) => {
            let decimal_digits = sanitized.len() - dot_pos - 1;
            if decimal_digits <= 2 {
                sanitized
            } else {
                sanitized.replace('.', "")
            }
        }
        (None, None) => sanitized,
    };

    let parsed: f64 = normalized.parse().unwrap_or(0.0);
    Some((parsed * 100.0).round() as i64)
}

/// Normalizes phone number to digits only
pub fn normalize_phone(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Normalizes patient name: lowercase, no accents, no extra spaces
pub fn normalize_patient_name(value: &str) -> String {
    let normalized = value
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' => 'a',
            'é' | 'è' | 'ê' => 'e',
            'í' | 'ì' | 'î' => 'i',
            'ó' | 'ò' | 'õ' | 'ô' => 'o',
            'ú' | 'ù' | 'û' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            _ => c,
        })
        .collect::<String>();

    let words: Vec<&str> = normalized.split_whitespace().collect();
    words.join(" ")
}

/// Builds a unique identity key for a patient
pub fn build_patient_identity_key(full_name: &str, phone: Option<&str>) -> String {
    format!(
        "{}::{}",
        normalize_patient_name(full_name),
        phone.map(normalize_phone).unwrap_or_default()
    )
}

/// Formats phone number for display: (11) 99999-8888
pub fn format_phone(value: &str) -> String {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        11 => format!(
            "({}) {}-{}",
            &digits[..2],
            &digits[2..7],
            &digits[7..]
        ),
        10 => format!(
            "({}) {}-{}",
            &digits[..2],
            &digits[2..6],
            &digits[6..]
        ),
        _ => value.to_string(),
    }
}

/// Parses Brazilian date format (dd/mm/yyyy) to ISO date (yyyy-mm-dd)
pub fn parse_date_br(value: &str) -> Option<String> {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 8 {
        return None;
    }
    let day: u32 = digits[..2].parse().ok()?;
    let month: u32 = digits[2..4].parse().ok()?;
    let year: i32 = digits[4..].parse().ok()?;

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(date.format("%Y-%m-%d").to_string())
}

/// Validates and formats dates
pub fn format_date_br(date: &str) -> String {
    if date.len() == 10 && date.chars().nth(4) == Some('-') {
        // ISO format: 2024-01-15 -> 15/01/2024
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 {
            return format!("{}/{}/{}", parts[2], parts[1], parts[0]);
        }
    }
    date.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_currency_brl() {
        assert_eq!(format_currency_brl(123456), "R$ 1.234,56");
        assert_eq!(format_currency_brl(0), "R$ 0,00");
        assert_eq!(format_currency_brl(99), "R$ 0,99");
        assert_eq!(format_currency_brl(15000), "R$ 150,00");
    }

    #[test]
    fn test_parse_brl_to_cents() {
        assert_eq!(parse_brl_to_cents("R$ 1.234,56"), Some(123456));
        assert_eq!(parse_brl_to_cents("R$ 0,00"), Some(0));
        assert_eq!(parse_brl_to_cents("150,00"), Some(15000));
        assert_eq!(parse_brl_to_cents("100"), Some(10000));
        assert_eq!(parse_brl_to_cents(""), Some(0));
    }

    #[test]
    fn test_normalize_patient_name() {
        assert_eq!(
            normalize_patient_name("João da Silva"),
            "joao da silva"
        );
        assert_eq!(
            normalize_patient_name("Maria José"),
            "maria jose"
        );
        assert_eq!(
            normalize_patient_name("  Ana   Beatriz  "),
            "ana beatriz"
        );
    }

    #[test]
    fn test_build_patient_identity_key() {
        let key = build_patient_identity_key("João Silva", Some("(11) 99999-8888"));
        assert_eq!(key, "joao silva::11999998888");
    }

    #[test]
    fn test_format_phone() {
        assert_eq!(format_phone("11999998888"), "(11) 99999-8888");
        assert_eq!(format_phone("1133334444"), "(11) 3333-4444");
    }

    #[test]
    fn test_parse_date_br() {
        assert_eq!(
            parse_date_br("15/01/2024"),
            Some("2024-01-15".to_string())
        );
        assert_eq!(parse_date_br("31/02/2024"), None); // invalid date
    }
}
