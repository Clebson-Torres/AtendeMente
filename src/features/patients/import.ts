import { z } from "zod";
import { buildPatientIdentityKey, parseDateInputBR } from "@/lib/utils";

const patientImportRowSchema = z.object({
  fullName: z.string().trim().min(1, "Nome obrigatorio."),
  phone: z.string().trim().optional().default(""),
  email: z.string().trim().optional().default(""),
  birthDate: z.string().trim().optional().default(""),
  emergencyPhone: z.string().trim().optional().default(""),
  medicationsInUse: z.string().trim().optional().default(""),
  healthHistory: z.string().trim().optional().default(""),
  adminNotes: z.string().trim().optional().default(""),
});

export type PatientImportRow = z.infer<typeof patientImportRowSchema>;
export type PatientImportPreviewRow = PatientImportRow & { rowNumber: number };

const headerAliases: Record<string, keyof PatientImportRow> = {
  nome: "fullName",
  nomecompleto: "fullName",
  nomedopaciente: "fullName",
  paciente: "fullName",
  full_name: "fullName",
  fullname: "fullName",
  telefone: "phone",
  telefonecelular: "phone",
  celular: "phone",
  phone: "phone",
  email: "email",
  nascimento: "birthDate",
  datadenascimento: "birthDate",
  dtnascimento: "birthDate",
  birthdate: "birthDate",
  telefoneemergencia: "emergencyPhone",
  telefonecontato: "emergencyPhone",
  emergencyphone: "emergencyPhone",
  medicamentos: "medicationsInUse",
  medicamentos_em_uso: "medicationsInUse",
  medicamentosemuso: "medicationsInUse",
  medicationsinuse: "medicationsInUse",
  historicodesaude: "healthHistory",
  historico: "healthHistory",
  healthhistory: "healthHistory",
  observacoes: "adminNotes",
  observacoesadministrativas: "adminNotes",
  adminnotes: "adminNotes",
};

function normalizeHeader(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]/g, "");
}

function parseCsvLine(line: string) {
  const result: string[] = [];
  let current = "";
  let inQuotes = false;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    const nextChar = line[index + 1];

    if (char === '"' && inQuotes && nextChar === '"') {
      current += '"';
      index += 1;
      continue;
    }

    if (char === '"') {
      inQuotes = !inQuotes;
      continue;
    }

    if (char === "," && !inQuotes) {
      result.push(current.trim());
      current = "";
      continue;
    }

    current += char;
  }

  result.push(current.trim());
  return result;
}

function isStructuralCsvLine(line: string) {
  return line.replace(/[",;\s]/g, "").length === 0;
}

export function parsePatientsCsv(csvContent: string) {
  const rawLines = csvContent
    .split(/\r?\n/)
    .map((line) => line.trim());

  const headerStartIndex = rawLines.findIndex((line) => line && !isStructuralCsvLine(line));

  if (headerStartIndex === -1) {
    return { rows: [], errors: [{ row: 0, message: "Arquivo CSV sem conteudo suficiente para importacao." }] };
  }

  const lines = rawLines.slice(headerStartIndex).filter((line) => line && !isStructuralCsvLine(line));

  if (lines.length < 2) {
    return { rows: [], errors: [{ row: 0, message: "Arquivo CSV sem conteudo suficiente para importacao." }] };
  }

  const headers = parseCsvLine(lines[0]).map((header) => headerAliases[normalizeHeader(header)] ?? null);
  const hasRecognizedNameColumn = headers.includes("fullName");
  const rows: PatientImportPreviewRow[] = [];
  const errors: Array<{ row: number; message: string }> = [];

  if (!hasRecognizedNameColumn) {
    return {
      rows: [],
      errors: [
        {
          row: 1,
          message: "Cabecalho sem coluna de nome reconhecida. Use, por exemplo: nome, nome completo ou paciente.",
        },
      ],
    };
  }

  for (let index = 1; index < lines.length; index += 1) {
    const values = parseCsvLine(lines[index]);
    const rawRow: Record<keyof PatientImportRow, string> = {
      fullName: "",
      phone: "",
      email: "",
      birthDate: "",
      emergencyPhone: "",
      medicationsInUse: "",
      healthHistory: "",
      adminNotes: "",
    };

    headers.forEach((header, headerIndex) => {
      if (!header) {
        return;
      }

      rawRow[header] = values[headerIndex] ?? "";
    });

    const parsed = patientImportRowSchema.safeParse(rawRow);

    if (!parsed.success) {
      const firstIssue = parsed.error.issues[0];
      errors.push({
        row: index + 1,
        message:
          firstIssue?.path?.[0] === "fullName" && !rawRow.fullName
            ? "Nome obrigatorio ou coluna de nome nao reconhecida."
            : firstIssue?.message ?? "Linha invalida.",
      });
      continue;
    }

    rows.push({
      ...parsed.data,
      rowNumber: index + 1,
    });
  }

  return { rows, errors };
}

export function buildPatientDuplicateKey(row: {
  fullName: string;
  phone?: string | null;
}) {
  return buildPatientIdentityKey(row);
}

export function detectImportedPatientDuplicates(rows: PatientImportPreviewRow[]) {
  const seen = new Map<string, number>();
  const duplicates: Array<{ row: number; duplicateOf: number; fullName: string }> = [];

  rows.forEach((row) => {
    const key = buildPatientDuplicateKey(row);

    if (!seen.has(key)) {
      seen.set(key, row.rowNumber);
      return;
    }

    duplicates.push({
      row: row.rowNumber,
      duplicateOf: seen.get(key)!,
      fullName: row.fullName,
    });
  });

  return duplicates;
}

export function detectExistingPatientDuplicates(
  importedRows: PatientImportPreviewRow[],
  existingPatients: Array<{ id: string; fullName: string; phone: string | null }>,
) {
  const existingMap = new Map(existingPatients.map((patient) => [buildPatientDuplicateKey(patient), patient]));

  return importedRows.flatMap((row) => {
    const existing = existingMap.get(buildPatientDuplicateKey(row));

    if (!existing) {
      return [];
    }

    return [
      {
        row: row.rowNumber,
        existingPatientId: existing.id,
        fullName: row.fullName,
      },
    ];
  });
}

export function normalizeImportedBirthDate(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "";
  }

  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
    return trimmed;
  }

  return parseDateInputBR(trimmed);
}

export async function parsePatientsSpreadsheet(file: File) {
  const extension = file.name.toLowerCase().split(".").pop();

  if (extension === "csv") {
    return parsePatientsCsv(await file.text());
  }

  return { rows: [], errors: [{ row: 0, message: "Formato de arquivo nao suportado. Use CSV." }] };
}
