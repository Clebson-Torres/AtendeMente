import "server-only";
import { and, asc, desc, eq, ilike, isNull, or, sql } from "drizzle-orm";
import { getDb } from "@/db/client";
import { appointments, patients, payments, recordFiles, recurringSeries, sessionRecords } from "@/db/schema";
import { getSeriesSummary } from "@/features/appointments/recurrence";
import { decryptRecordContent } from "@/lib/crypto/records";
import { AppError } from "@/lib/errors/app-error";

function isMissingPatientProfileColumnError(error: unknown) {
  const candidate =
    typeof error === "object" && error !== null && "cause" in error
      ? error.cause
      : error;

  return (
    typeof candidate === "object" &&
    candidate !== null &&
    "code" in candidate &&
    candidate.code === "42703"
  );
}

function isMissingRecurringSchemaError(error: unknown) {
  const candidate =
    typeof error === "object" && error !== null && "cause" in error
      ? error.cause
      : error;

  return (
    typeof candidate === "object" &&
    candidate !== null &&
    "code" in candidate &&
    (candidate.code === "42703" || candidate.code === "42P01")
  );
}

export async function listPatients(userId: string, search = "") {
  const filters = [eq(patients.userId, userId), isNull(patients.deletedAt)];
  const trimmedSearch = search.trim();
  const normalizedPhoneSearch = trimmedSearch.replace(/\D/g, "");

  if (trimmedSearch) {
    filters.push(
      or(
        ilike(patients.fullName, `%${trimmedSearch}%`),
        ilike(sql`coalesce(${patients.email}, '')`, `%${trimmedSearch}%`),
        normalizedPhoneSearch
          ? sql<boolean>`regexp_replace(coalesce(${patients.phone}, ''), '\D', '', 'g') like ${`%${normalizedPhoneSearch}%`}`
          : ilike(sql`coalesce(${patients.phone}, '')`, `%${trimmedSearch}%`),
      )!,
    );
  }

  const db = getDb();

  try {
    return await db
      .select({
        id: patients.id,
        fullName: patients.fullName,
        chartNumber: patients.chartNumber,
        phone: patients.phone,
        emergencyPhone: patients.emergencyPhone,
        email: patients.email,
        birthDate: patients.birthDate,
        status: patients.status,
        createdAt: patients.createdAt,
      })
      .from(patients)
      .where(and(...filters))
      .orderBy(patients.fullName);
  } catch (error) {
    if (!isMissingPatientProfileColumnError(error)) {
      throw error;
    }

    const legacyPatients = await db
      .select({
        id: patients.id,
        fullName: patients.fullName,
        chartNumber: sql<string | null>`null`,
        phone: patients.phone,
        email: patients.email,
        birthDate: patients.birthDate,
        status: sql<"active" | "inactive">`'active'`,
        createdAt: patients.createdAt,
      })
      .from(patients)
      .where(and(...filters))
      .orderBy(patients.fullName);

    return legacyPatients.map((patient) => ({
      ...patient,
      emergencyPhone: null,
    }));
  }
}

export async function getPatientDetail(userId: string, patientId: string) {
  const db = getDb();
  let patient:
    | {
        id: string;
        userId: string;
        fullName: string;
        chartNumber: string | null;
        phone: string | null;
        email: string | null;
        birthDate: string | null;
        status: "active" | "inactive";
        healthHistory: string | null;
        medicationsInUse: string | null;
        emergencyPhone: string | null;
        adminNotes: string | null;
        createdAt: Date;
        updatedAt: Date;
        deletedAt: Date | null;
      }
    | undefined;

  try {
    [patient] = await db
      .select({
        id: patients.id,
        userId: patients.userId,
        fullName: patients.fullName,
        chartNumber: patients.chartNumber,
        phone: patients.phone,
        email: patients.email,
        birthDate: patients.birthDate,
        status: patients.status,
        healthHistory: patients.healthHistory,
        medicationsInUse: patients.medicationsInUse,
        emergencyPhone: patients.emergencyPhone,
        adminNotes: patients.adminNotes,
        createdAt: patients.createdAt,
        updatedAt: patients.updatedAt,
        deletedAt: patients.deletedAt,
      })
      .from(patients)
      .where(and(eq(patients.userId, userId), eq(patients.id, patientId), isNull(patients.deletedAt)))
      .limit(1);
  } catch (error) {
    if (!isMissingPatientProfileColumnError(error)) {
      throw error;
    }

    const [legacyPatient] = await db
      .select({
        id: patients.id,
        userId: patients.userId,
        fullName: patients.fullName,
        chartNumber: sql<string | null>`null`,
        phone: patients.phone,
        email: patients.email,
        birthDate: patients.birthDate,
        status: sql<"active" | "inactive">`'active'`,
        adminNotes: patients.adminNotes,
        createdAt: patients.createdAt,
        updatedAt: patients.updatedAt,
        deletedAt: patients.deletedAt,
      })
      .from(patients)
      .where(and(eq(patients.userId, userId), eq(patients.id, patientId), isNull(patients.deletedAt)))
      .limit(1);

    patient = legacyPatient
      ? {
          ...legacyPatient,
          status: legacyPatient.status,
          healthHistory: null,
          medicationsInUse: null,
          emergencyPhone: null,
        }
      : undefined;
  }

  if (!patient) {
    throw new AppError("Paciente nao encontrado.", { statusCode: 404, code: "PATIENT_NOT_FOUND" });
  }

  let appointmentRows: Array<{
    appointmentId: string;
    startsAt: Date;
    endsAt: Date;
    status: "scheduled" | "completed" | "cancelled" | "no_show";
    confirmationStatus: "unconfirmed" | "confirmed" | "cancelled";
    seriesId: string | null;
    sessionPriceCents: number;
    quickNotes: string | null;
    paymentStatus: "pending" | "paid" | "cancelled" | null;
    paymentMethod: "pix" | "cash" | "card" | "bank_transfer" | "other" | null;
    amountReceivedCents: number | null;
    paidAt: Date | null;
    recordId: string | null;
    encryptedPayload: string | null;
    iv: string | null;
    authTag: string | null;
    keyVersion: number | null;
  }> = [];

  try {
    appointmentRows = await db
      .select({
        appointmentId: appointments.id,
        startsAt: appointments.startsAt,
        endsAt: appointments.endsAt,
        status: appointments.status,
        confirmationStatus: appointments.confirmationStatus,
        seriesId: appointments.seriesId,
        sessionPriceCents: appointments.sessionPriceCents,
        quickNotes: appointments.quickNotes,
        paymentStatus: payments.status,
        paymentMethod: payments.method,
        amountReceivedCents: payments.amountReceivedCents,
        paidAt: payments.paidAt,
        recordId: sessionRecords.id,
        encryptedPayload: sessionRecords.encryptedPayload,
        iv: sessionRecords.iv,
        authTag: sessionRecords.authTag,
        keyVersion: sessionRecords.keyVersion,
      })
      .from(appointments)
      .leftJoin(
        payments,
        and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)),
      )
      .leftJoin(
        sessionRecords,
        and(eq(sessionRecords.appointmentId, appointments.id), isNull(sessionRecords.deletedAt)),
      )
      .where(
        and(
          eq(appointments.userId, userId),
          eq(appointments.patientId, patientId),
          isNull(appointments.deletedAt),
        ),
      )
      .orderBy(desc(appointments.startsAt));
  } catch (error) {
    if (!isMissingRecurringSchemaError(error)) {
      throw error;
    }

    appointmentRows = await db
      .select({
        appointmentId: appointments.id,
        startsAt: appointments.startsAt,
        endsAt: appointments.endsAt,
        status: appointments.status,
        confirmationStatus: appointments.confirmationStatus,
        sessionPriceCents: appointments.sessionPriceCents,
        quickNotes: appointments.quickNotes,
        paymentStatus: payments.status,
        paymentMethod: payments.method,
        amountReceivedCents: payments.amountReceivedCents,
        paidAt: payments.paidAt,
        recordId: sessionRecords.id,
        encryptedPayload: sessionRecords.encryptedPayload,
        iv: sessionRecords.iv,
        authTag: sessionRecords.authTag,
        keyVersion: sessionRecords.keyVersion,
      })
      .from(appointments)
      .leftJoin(
        payments,
        and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)),
      )
      .leftJoin(
        sessionRecords,
        and(eq(sessionRecords.appointmentId, appointments.id), isNull(sessionRecords.deletedAt)),
      )
      .where(
        and(
          eq(appointments.userId, userId),
          eq(appointments.patientId, patientId),
          isNull(appointments.deletedAt),
        ),
      )
      .orderBy(desc(appointments.startsAt))
      .then((rows) => rows.map((row) => ({ ...row, seriesId: null })));
  }

  const files = await db
    .select({
      id: recordFiles.id,
      appointmentId: recordFiles.appointmentId,
      paymentId: recordFiles.paymentId,
      originalName: recordFiles.originalName,
      mimeType: recordFiles.mimeType,
      byteSize: recordFiles.byteSize,
      uploadedAt: recordFiles.uploadedAt,
      kind: recordFiles.kind,
    })
    .from(recordFiles)
    .where(
      and(
        eq(recordFiles.userId, userId),
        eq(recordFiles.patientId, patientId),
        isNull(recordFiles.deletedAt),
      ),
    )
    .orderBy(desc(recordFiles.uploadedAt));

  let recurring: Array<{
    id: string;
    frequency: "weekly" | "biweekly";
    startsOn: string;
    endsOn: string | null;
    occurrencesCount: number | null;
    startTime: string;
    endTime: string;
    cancelledAt: Date | null;
  }> = [];

  try {
    recurring = await db
      .select({
        id: recurringSeries.id,
        frequency: recurringSeries.frequency,
        startsOn: recurringSeries.startsOn,
        endsOn: recurringSeries.endsOn,
        occurrencesCount: recurringSeries.occurrencesCount,
        startTime: recurringSeries.startTime,
        endTime: recurringSeries.endTime,
        cancelledAt: recurringSeries.cancelledAt,
      })
      .from(recurringSeries)
      .where(
        and(
          eq(recurringSeries.userId, userId),
          eq(recurringSeries.patientId, patientId),
        ),
      )
      .orderBy(asc(recurringSeries.createdAt));
  } catch (error) {
    if (!isMissingRecurringSchemaError(error)) {
      throw error;
    }
  }

  const filesByAppointment = files.reduce<Record<string, typeof files>>((acc, file) => {
    acc[file.appointmentId] ??= [];
    acc[file.appointmentId].push(file);
    return acc;
  }, {});

  return {
    patient,
    timeline: appointmentRows.map((row) => ({
      appointmentId: row.appointmentId,
      startsAt: row.startsAt,
      endsAt: row.endsAt,
      status: row.status,
      confirmationStatus: row.confirmationStatus,
      seriesId: row.seriesId,
      sessionPriceCents: row.sessionPriceCents,
      quickNotes: row.quickNotes,
      paymentStatus: row.paymentStatus,
      paymentMethod: row.paymentMethod,
      amountReceivedCents: row.amountReceivedCents,
      paidAt: row.paidAt,
      summary:
        row.recordId && row.encryptedPayload
          ? decryptRecordContent({
              encryptedPayload: row.encryptedPayload,
              iv: row.iv!,
              authTag: row.authTag!,
              keyVersion: row.keyVersion!,
            })
          : null,
      files: filesByAppointment[row.appointmentId] ?? [],
    })),
    recurringSeries: recurring.map((series) => ({
      ...series,
      summary: getSeriesSummary(series),
    })),
  };
}
