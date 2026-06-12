import { z } from "zod";

export const loginSchema = z.object({
  email: z.string().email("Email inválido"),
  password: z.string().min(1, "Senha é obrigatória"),
});

export const registerSchema = z.object({
  full_name: z.string().min(2, "Nome deve ter no mínimo 2 caracteres"),
  email: z.string().email("Email inválido"),
  password: z.string().min(6, "Senha deve ter no mínimo 6 caracteres"),
});

export const patientSchema = z.object({
  full_name: z.string().min(2, "Nome deve ter no mínimo 2 caracteres"),
  chart_number: z.string().optional(),
  phone: z.string().optional(),
  email: z.union([z.string().email("Email inválido"), z.literal("")]).optional(),
  birth_date: z.string().optional(),
  emergency_phone: z.string().optional(),
  health_history: z.string().optional(),
  medications_in_use: z.string().optional(),
  admin_notes: z.string().optional(),
});

export const appointmentSchema = z.object({
  patient_id: z.string().min(1, "Selecione um paciente"),
  starts_at: z.string().min(1, "Informe o início"),
  ends_at: z.string().min(1, "Informe o fim"),
  session_price_cents: z.number().optional(),
});

export const paymentSchema = z.object({
  status: z.string().min(1, "Selecione o status"),
  method: z.string().min(1, "Selecione o método"),
  amount_received_cents: z.number().min(0, "Valor deve ser positivo"),
  notes: z.string().optional(),
  paid_at: z.string().nullable().optional(),
});

export const resetPasswordSchema = z.object({
  new_password: z.string().min(6, "Senha deve ter no mínimo 6 caracteres"),
});

export type LoginInput = z.infer<typeof loginSchema>;
export type RegisterInput = z.infer<typeof registerSchema>;
export type PatientInput = z.infer<typeof patientSchema>;
export type AppointmentInput = z.infer<typeof appointmentSchema>;
export type PaymentInput = z.infer<typeof paymentSchema>;
