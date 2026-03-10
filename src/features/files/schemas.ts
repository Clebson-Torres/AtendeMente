import { z } from "zod";
import { recordFileKinds } from "@/types/domain";

export const allowedFileExtensions = [".pdf", ".doc", ".docx", ".png", ".jpg", ".jpeg"] as const;

const allowedMimeTypes = [
  "application/pdf",
  "application/msword",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "image/png",
  "image/jpeg",
] as const;

export const fileUploadRequestSchema = z.object({
  appointmentId: z.uuid("Atendimento invalido."),
  patientId: z.uuid("Paciente invalido."),
  paymentId: z.uuid("Pagamento invalido.").optional(),
  kind: z.enum(recordFileKinds).default("session_attachment"),
  fileName: z.string().trim().min(1).max(255),
  fileSize: z.number().int().positive().max(10 * 1024 * 1024, "Arquivo acima de 10 MB."),
  mimeType: z.enum(allowedMimeTypes, "Formato de arquivo nao permitido."),
}).superRefine((value, ctx) => {
  const normalizedName = value.fileName.toLowerCase();
  const hasAllowedExtension = allowedFileExtensions.some((extension) => normalizedName.endsWith(extension));

  if (!hasAllowedExtension) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: "Extensao de arquivo nao permitida.",
      path: ["fileName"],
    });
  }

  if (value.kind === "payment_receipt" && !value.paymentId) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: "Informe o pagamento vinculado ao recibo.",
      path: ["paymentId"],
    });
  }
});

export const confirmUploadSchema = z.object({
  fileId: z.uuid("Arquivo invalido."),
});

export type FileUploadRequest = z.infer<typeof fileUploadRequestSchema>;
