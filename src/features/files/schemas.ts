import { z } from "zod";
import { recordFileKinds } from "@/types/domain";

export const allowedFileExtensions = [".pdf", ".doc", ".docx", ".png", ".jpg", ".jpeg"] as const;

export const allowedMimeTypes = [
  "application/pdf",
  "application/msword",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "image/png",
  "image/jpeg",
  "image/jpg",
] as const;

export const allowedMimeTypesByExtension = {
  ".pdf": ["application/pdf"],
  ".doc": ["application/msword"],
  ".docx": ["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
  ".png": ["image/png"],
  ".jpg": ["image/jpeg", "image/jpg"],
  ".jpeg": ["image/jpeg", "image/jpg"],
} as const;

export function getFileExtension(fileName: string) {
  const normalizedName = fileName.trim().toLowerCase();
  const matchedExtension = allowedFileExtensions.find((extension) => normalizedName.endsWith(extension));
  return matchedExtension ?? null;
}

export function isAllowedMimeTypeForFile(fileName: string, mimeType: string) {
  const extension = getFileExtension(fileName);

  if (!extension) {
    return false;
  }

  const normalizedMimeType = mimeType.trim().toLowerCase();
  const allowedMimeTypesForExtension = allowedMimeTypesByExtension[extension] as readonly string[];
  return allowedMimeTypesForExtension.includes(normalizedMimeType);
}

export const fileUploadRequestSchema = z.object({
  appointmentId: z.uuid("Atendimento invalido."),
  patientId: z.uuid("Paciente invalido."),
  paymentId: z.uuid("Pagamento invalido.").optional(),
  kind: z.enum(recordFileKinds).default("session_attachment"),
  fileName: z.string().trim().min(1).max(255),
  fileSize: z.number().int().positive().max(10 * 1024 * 1024, "Arquivo acima de 10 MB."),
  mimeType: z.enum(allowedMimeTypes, "Formato de arquivo nao permitido."),
}).superRefine((value, ctx) => {
  const hasAllowedExtension = Boolean(getFileExtension(value.fileName));

  if (!hasAllowedExtension) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: "Extensao de arquivo nao permitida.",
      path: ["fileName"],
    });
  }

  if (!isAllowedMimeTypeForFile(value.fileName, value.mimeType)) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: "O tipo do arquivo nao corresponde a extensao informada.",
      path: ["mimeType"],
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
