import { describe, it, expect } from "vitest";
import {
  loginSchema,
  registerSchema,
  patientSchema,
  appointmentSchema,
  paymentSchema,
  resetPasswordSchema,
} from "../src/lib/schemas";

describe("loginSchema", () => {
  it("validates correct input", () => {
    const result = loginSchema.safeParse({ email: "test@test.com", password: "12345678" });
    expect(result.success).toBe(true);
  });

  it("rejects invalid email", () => {
    const result = loginSchema.safeParse({ email: "invalid", password: "12345678" });
    expect(result.success).toBe(false);
  });

  it("rejects empty password", () => {
    const result = loginSchema.safeParse({ email: "test@test.com", password: "" });
    expect(result.success).toBe(false);
  });
});

describe("registerSchema", () => {
  it("validates correct input", () => {
    const result = registerSchema.safeParse({
      full_name: "João Silva",
      email: "joao@test.com",
      password: "12345678",
    });
    expect(result.success).toBe(true);
  });

  it("rejects short name", () => {
    const result = registerSchema.safeParse({
      full_name: "J",
      email: "joao@test.com",
      password: "12345678",
    });
    expect(result.success).toBe(false);
  });

  it("rejects short password", () => {
    const result = registerSchema.safeParse({
      full_name: "João Silva",
      email: "joao@test.com",
      password: "1234567",
    });
    expect(result.success).toBe(false);
  });
});

describe("patientSchema", () => {
  it("validates minimum required fields", () => {
    const result = patientSchema.safeParse({ full_name: "Maria" });
    expect(result.success).toBe(true);
  });

  it("rejects empty name", () => {
    const result = patientSchema.safeParse({ full_name: "" });
    expect(result.success).toBe(false);
  });

  it("validates with all optional fields", () => {
    const result = patientSchema.safeParse({
      full_name: "Maria Souza",
      chart_number: "001",
      phone: "11999999999",
      email: "maria@test.com",
      birth_date: "1990-01-01",
      emergency_phone: "11988888888",
      health_history: "Nada relevante",
      medications_in_use: "Nenhum",
      admin_notes: "Observação",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty email", () => {
    const result = patientSchema.safeParse({ full_name: "Maria", email: "" });
    expect(result.success).toBe(true);
  });
});

describe("appointmentSchema", () => {
  it("validates correct input", () => {
    const result = appointmentSchema.safeParse({
      patient_id: "abc123",
      starts_at: "2024-03-15T10:00",
      ends_at: "2024-03-15T11:00",
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing patient", () => {
    const result = appointmentSchema.safeParse({
      patient_id: "",
      starts_at: "2024-03-15T10:00",
      ends_at: "2024-03-15T11:00",
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing dates", () => {
    const result = appointmentSchema.safeParse({
      patient_id: "abc123",
      starts_at: "",
      ends_at: "",
    });
    expect(result.success).toBe(false);
  });
});

describe("paymentSchema", () => {
  it("validates correct input", () => {
    const result = paymentSchema.safeParse({
      status: "paid",
      method: "pix",
      amount_received_cents: 15000,
    });
    expect(result.success).toBe(true);
  });

  it("rejects negative amount", () => {
    const result = paymentSchema.safeParse({
      status: "paid",
      method: "pix",
      amount_received_cents: -1,
    });
    expect(result.success).toBe(false);
  });
});

describe("resetPasswordSchema", () => {
  it("validates correct input", () => {
    const result = resetPasswordSchema.safeParse({ new_password: "12345678" });
    expect(result.success).toBe(true);
  });

  it("rejects short password", () => {
    const result = resetPasswordSchema.safeParse({ new_password: "1234567" });
    expect(result.success).toBe(false);
  });
});
