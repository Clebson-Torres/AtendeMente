import "server-only";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { and, eq, isNull } from "drizzle-orm";
import { getDb } from "@/db/client";
import { appointments, payments, recordFiles } from "@/db/schema";
import { allowedFileExtensions, allowedMimeTypes } from "@/features/files/schemas";
import { writeAuditLog } from "@/lib/audit/log";
import { getStorageBucket } from "@/lib/env";
import { AppError } from "@/lib/errors/app-error";
import { createAdminSupabaseClient } from "@/lib/supabase/admin";
import type { FileUploadRequest } from "@/features/files/schemas";

function makeStoragePath(userId: string, patientId: string, appointmentId: string, fileName: string) {
  const ext = path.extname(fileName).toLowerCase();
  return `${userId}/${patientId}/${appointmentId}/${randomUUID()}${ext}`;
}

function getNormalizedExtension(fileName: string) {
  return path.extname(fileName).toLowerCase();
}

function getStoredMimeType(fileInfo: { content_type?: string | null; metadata?: { mimetype?: string | null } | null }) {
  return fileInfo.content_type ?? fileInfo.metadata?.mimetype ?? "";
}

async function rejectUploadedFile(fileId: string, storagePath: string) {
  const adminClient = createAdminSupabaseClient();
  await adminClient.storage.from(getStorageBucket()).remove([storagePath]);
  await getDb()
    .update(recordFiles)
    .set({
      deletedAt: new Date(),
    })
    .where(eq(recordFiles.id, fileId));
}

export async function createUploadSession(userId: string, input: FileUploadRequest) {
  const db = getDb();

  if (!hasAllowedExtension(input.fileName)) {
    throw new AppError("Extensao de arquivo nao permitida.", { statusCode: 400 });
  }

  const [appointment] = await db
    .select({
      id: appointments.id,
      patientId: appointments.patientId,
    })
    .from(appointments)
    .where(
      and(
        eq(appointments.id, input.appointmentId),
        eq(appointments.userId, userId),
        eq(appointments.patientId, input.patientId),
        isNull(appointments.deletedAt),
      ),
    );

  if (!appointment) {
    throw new AppError("Atendimento nao encontrado.", { statusCode: 404 });
  }

  let paymentId: string | null = null;

  if (input.kind === "payment_receipt") {
    const [payment] = await db
      .select({
        id: payments.id,
        appointmentId: payments.appointmentId,
      })
      .from(payments)
      .where(
        and(
          eq(payments.id, input.paymentId!),
          eq(payments.userId, userId),
          eq(payments.appointmentId, input.appointmentId),
          isNull(payments.deletedAt),
        ),
      );

    if (!payment) {
      throw new AppError("Pagamento nao encontrado para vincular o recibo.", { statusCode: 404 });
    }

    paymentId = payment.id;
  }

  const storagePath = makeStoragePath(userId, input.patientId, input.appointmentId, input.fileName);
  const [file] = await db
    .insert(recordFiles)
    .values({
      userId,
      patientId: input.patientId,
      appointmentId: input.appointmentId,
      paymentId,
      kind: input.kind,
      storagePath,
      originalName: input.fileName,
      mimeType: input.mimeType,
      byteSize: input.fileSize,
    })
    .returning({
      id: recordFiles.id,
      storagePath: recordFiles.storagePath,
    });

  const adminClient = createAdminSupabaseClient();
  const { data, error } = await adminClient.storage
    .from(getStorageBucket())
    .createSignedUploadUrl(storagePath);

  if (error || !data) {
    throw new AppError("Nao foi possivel iniciar o upload do arquivo.", { statusCode: 500 });
  }

  return {
    fileId: file.id,
    path: data.path,
    token: data.token,
    signedUrl: data.signedUrl,
  };
}

export async function confirmUpload(userId: string, fileId: string) {
  const db = getDb();
  const [file] = await db
    .select({
      id: recordFiles.id,
      appointmentId: recordFiles.appointmentId,
      patientId: recordFiles.patientId,
      paymentId: recordFiles.paymentId,
      kind: recordFiles.kind,
      originalName: recordFiles.originalName,
      storagePath: recordFiles.storagePath,
      mimeType: recordFiles.mimeType,
      byteSize: recordFiles.byteSize,
    })
    .from(recordFiles)
    .where(and(eq(recordFiles.id, fileId), eq(recordFiles.userId, userId), isNull(recordFiles.deletedAt)));

  if (!file) {
    throw new AppError("Arquivo nao encontrado.", { statusCode: 404 });
  }

  const adminClient = createAdminSupabaseClient();
  const { data: uploadedFile, error } = await adminClient.storage.from(getStorageBucket()).info(file.storagePath);

  if (error || !uploadedFile) {
    throw new AppError("Nao foi possivel validar o arquivo enviado.", { statusCode: 400 });
  }

  const storedMimeType = getStoredMimeType(uploadedFile);
  const storedSize = uploadedFile.size ?? uploadedFile.metadata?.size ?? 0;
  const hasAllowedExtension = allowedFileExtensions.includes(
    getNormalizedExtension(file.originalName) as (typeof allowedFileExtensions)[number],
  );
  const hasAllowedMimeType = allowedMimeTypes.includes(
    storedMimeType as (typeof allowedMimeTypes)[number],
  );
  const matchesExpectedFile = storedMimeType === file.mimeType && storedSize === file.byteSize;

  if (!hasAllowedExtension || !hasAllowedMimeType || !matchesExpectedFile) {
    await rejectUploadedFile(file.id, file.storagePath);
    throw new AppError("Arquivo rejeitado pela validacao de seguranca.", {
      statusCode: 400,
      code: "FILE_VALIDATION_FAILED",
    });
  }

  await writeAuditLog({
    userId,
    action: "file_upload",
    entityType: "record_file",
    entityId: file.id,
    metadata: {
      kind: file.kind,
      originalName: file.originalName,
      appointmentId: file.appointmentId,
      patientId: file.patientId,
      paymentId: file.paymentId,
    },
  });

  return file;
}

export async function downloadFile(userId: string, fileId: string) {
  const db = getDb();
  const [file] = await db
    .select()
    .from(recordFiles)
    .where(and(eq(recordFiles.id, fileId), eq(recordFiles.userId, userId), isNull(recordFiles.deletedAt)));

  if (!file) {
    throw new AppError("Arquivo nao encontrado.", { statusCode: 404 });
  }

  const adminClient = createAdminSupabaseClient();
  const { data, error } = await adminClient.storage.from(getStorageBucket()).download(file.storagePath);

  if (error || !data) {
    throw new AppError("Nao foi possivel baixar o arquivo.", { statusCode: 500 });
  }

  await writeAuditLog({
    userId,
    action: "file_download",
    entityType: "record_file",
    entityId: file.id,
    metadata: {
      kind: file.kind,
      originalName: file.originalName,
      appointmentId: file.appointmentId,
      patientId: file.patientId,
      paymentId: file.paymentId,
    },
  });

  return { file, data };
}
