import { describe, it, expect } from "vitest";
import {
  formatBRL,
  formatDate,
  formatTime,
  formatPhone,
  formatCentsForInput,
  parseBRLToCents,
  maskDateInputBR,
  maskTimeInput,
  parseDateInputBR,
} from "../src/lib/format";

describe("formatBRL", () => {
  it("formats cents as BRL currency", () => {
    expect(formatBRL(15000)).toBe("R$ 150,00");
    expect(formatBRL(0)).toBe("R$ 0,00");
    expect(formatBRL(99)).toBe("R$ 0,99");
    expect(formatBRL(123456)).toBe("R$ 1.234,56");
  });
});

describe("formatDate", () => {
  it("formats ISO date to pt-BR", () => {
    expect(formatDate("2024-03-15T10:30:00")).toBe("15/03/2024");
  });

  it("returns dash for null", () => {
    expect(formatDate(null)).toBe("-");
  });
});

describe("formatTime", () => {
  it("extracts HH:mm from ISO string", () => {
    expect(formatTime("2024-03-15T10:30:00")).toBe("10:30");
  });

  it("returns dash for null", () => {
    expect(formatTime(null)).toBe("-");
  });
});

describe("formatPhone", () => {
  it("formats 11-digit mobile phone", () => {
    expect(formatPhone("11987654321")).toBe("(11) 98765-4321");
  });

  it("formats 10-digit landline phone", () => {
    expect(formatPhone("1133334444")).toBe("(11) 3333-4444");
  });

  it("returns dash for null/undefined", () => {
    expect(formatPhone(null)).toBe("-");
    expect(formatPhone(undefined)).toBe("-");
  });

  it("returns raw value when not matching patterns", () => {
    expect(formatPhone("123")).toBe("123");
  });
});

describe("formatCentsForInput", () => {
  it("formats cents with BR locale", () => {
    const result = formatCentsForInput(15000);
    expect(result).toMatch(/150,00/);
  });

  it("handles null/undefined", () => {
    expect(formatCentsForInput(null)).toMatch(/0,00/);
    expect(formatCentsForInput(undefined)).toMatch(/0,00/);
  });
});

describe("parseBRLToCents", () => {
  it("parses numeric string to cents", () => {
    expect(parseBRLToCents("150,00")).toBe(15000);
    expect(parseBRLToCents("1.234,56")).toBe(123456);
  });

  it("parses number value", () => {
    expect(parseBRLToCents(150)).toBe(15000);
    expect(parseBRLToCents(150.5)).toBe(15050);
  });

  it("returns 0 for null/undefined/empty", () => {
    expect(parseBRLToCents(null)).toBe(0);
    expect(parseBRLToCents(undefined)).toBe(0);
    expect(parseBRLToCents("")).toBe(0);
  });
});

describe("maskDateInputBR", () => {
  it("masks digits as dd/mm/aaaa", () => {
    expect(maskDateInputBR("15032024")).toBe("15/03/2024");
    expect(maskDateInputBR("1503")).toBe("15/03");
    expect(maskDateInputBR("15")).toBe("15");
  });

  it("ignores non-digit chars", () => {
    expect(maskDateInputBR("15/03/2024")).toBe("15/03/2024");
  });
});

describe("maskTimeInput", () => {
  it("masks digits as HH:mm", () => {
    expect(maskTimeInput("1430")).toBe("14:30");
    expect(maskTimeInput("14")).toBe("14");
  });
});

describe("parseDateInputBR", () => {
  it("parses BR date to ISO date string", () => {
    expect(parseDateInputBR("15/03/2024")).toBe("2024-03-15");
  });

  it("returns empty for invalid format", () => {
    expect(parseDateInputBR("15-03-2024")).toBe("");
    expect(parseDateInputBR("")).toBe("");
  });
});
