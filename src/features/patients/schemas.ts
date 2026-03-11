import { z } from "zod";

export const patientFormSchema = z.object({
  fullName: z.string().trim().min(3, "Informe o nome completo."),
  chartNumber: z.string().trim().max(64, "Numero do prontuario muito longo.").optional().or(z.literal("")),
  phone: z.string().trim().max(20).optional().or(z.literal("")),
  email: z.email("Informe um email valido.").optional().or(z.literal("")),
  birthDate: z.string().optional().or(z.literal("")),
  healthHistory: z.string().max(5_000).optional().or(z.literal("")),
  medicationsInUse: z.string().max(5_000).optional().or(z.literal("")),
  emergencyPhone: z.string().trim().max(20).optional().or(z.literal("")),
  adminNotes: z.string().max(5_000).optional().or(z.literal("")),
});

export type PatientFormInput = z.infer<typeof patientFormSchema>;
