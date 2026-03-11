import { describe, expect, it } from "vitest";
import { patientFormSchema } from "@/features/patients/schemas";

describe("patient schema", () => {
  it("accepts a valid patient payload", () => {
    const parsed = patientFormSchema.safeParse({
      fullName: "Marina Alves",
      chartNumber: "PR-2026-001",
      phone: "11999998888",
      email: "marina@email.com",
      birthDate: "1992-08-12",
      adminNotes: "Prefere contato por WhatsApp.",
    });

    expect(parsed.success).toBe(true);
  });

  it("rejects a too-short name", () => {
    const parsed = patientFormSchema.safeParse({
      fullName: "Ma",
    });

    expect(parsed.success).toBe(false);
  });

  it("accepts an empty chart number", () => {
    const parsed = patientFormSchema.safeParse({
      fullName: "Marina Alves",
      chartNumber: "",
    });

    expect(parsed.success).toBe(true);
  });

  it("trims a valid chart number", () => {
    const parsed = patientFormSchema.parse({
      fullName: "Marina Alves",
      chartNumber: "  PR-0007  ",
    });

    expect(parsed.chartNumber).toBe("PR-0007");
  });

  it("rejects a chart number above the max length", () => {
    const parsed = patientFormSchema.safeParse({
      fullName: "Marina Alves",
      chartNumber: "A".repeat(65),
    });

    expect(parsed.success).toBe(false);
  });
});
