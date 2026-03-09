import { describe, expect, it } from "vitest";
import {
  detectExistingPatientDuplicates,
  detectImportedPatientDuplicates,
  normalizeImportedBirthDate,
  parsePatientsCsv,
  parsePatientsSpreadsheet,
} from "@/features/patients/import";

describe("patient import", () => {
  it("parses CSV rows using the supported template headers", () => {
    const csv = [
      "nome,telefone,email,observacoes",
      "Maria Silva,(11)99999-1111,maria@email.com,Paciente recorrente",
    ].join("\n");

    const preview = parsePatientsCsv(csv);

    expect(preview.errors).toHaveLength(0);
    expect(preview.rows[0]?.fullName).toBe("Maria Silva");
    expect(preview.rows[0]?.adminNotes).toBe("Paciente recorrente");
  });

  it("returns a clear error when the name column is not recognized", () => {
    const csv = [
      "cliente,telefone,email",
      "Maria Silva,(11)99999-1111,maria@email.com",
    ].join("\n");

    const preview = parsePatientsCsv(csv);

    expect(preview.rows).toHaveLength(0);
    expect(preview.errors[0]?.message).toContain("coluna de nome");
  });

  it("ignores leading CSV lines with only separators before the header", () => {
    const csv = [
      ",",
      ",",
      "",
      "Nome,data de nascimento",
      "Evandro Evangelista Do Nascimento,27/05/1992",
    ].join("\n");

    const preview = parsePatientsCsv(csv);

    expect(preview.errors).toHaveLength(0);
    expect(preview.rows).toHaveLength(1);
    expect(preview.rows[0]?.fullName).toBe("Evandro Evangelista Do Nascimento");
    expect(preview.rows[0]?.birthDate).toBe("27/05/1992");
  });

  it("normalizes imported birth dates from brazilian format to iso", () => {
    expect(normalizeImportedBirthDate("27/05/1992")).toBe("1992-05-27");
    expect(normalizeImportedBirthDate("1992-05-27")).toBe("1992-05-27");
    expect(normalizeImportedBirthDate("")).toBe("");
  });

  it("detects duplicate imported patients", () => {
    const rows = [
      {
        rowNumber: 2,
        fullName: "Maria Silva",
        phone: "(11)99999-1111",
        email: "maria@email.com",
        birthDate: "",
        emergencyPhone: "",
        medicationsInUse: "",
        healthHistory: "",
        adminNotes: "",
      },
      {
        rowNumber: 3,
        fullName: "Maria Silva",
        phone: "11 99999-1111",
        email: "outro@email.com",
        birthDate: "",
        emergencyPhone: "",
        medicationsInUse: "",
        healthHistory: "",
        adminNotes: "",
      },
    ];

    const duplicates = detectImportedPatientDuplicates(rows);

    expect(duplicates).toHaveLength(1);
    expect(duplicates[0]?.duplicateOf).toBe(2);
  });

  it("detects duplicates against existing patients", () => {
    const duplicates = detectExistingPatientDuplicates(
      [
        {
          rowNumber: 2,
          fullName: "Maria Silva",
          phone: "(11)99999-1111",
          email: "maria@email.com",
          birthDate: "",
          emergencyPhone: "",
          medicationsInUse: "",
          healthHistory: "",
          adminNotes: "",
        },
      ],
      [
        {
          id: "patient-1",
          fullName: "Maria Silva",
          phone: "11999991111",
        },
      ],
    );

    expect(duplicates).toHaveLength(1);
    expect(duplicates[0]?.existingPatientId).toBe("patient-1");
  });

  it("rejects non-csv spreadsheet uploads", async () => {
    const file = new File(["conteudo"], "pacientes.xlsx", {
      type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    });

    const preview = await parsePatientsSpreadsheet(file);

    expect(preview.rows).toHaveLength(0);
    expect(preview.errors[0]?.message).toContain("Use CSV");
  });
});
