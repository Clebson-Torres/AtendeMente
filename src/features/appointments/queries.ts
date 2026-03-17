import "server-only";
import { and, asc, desc, eq, gte, isNull, lte } from "drizzle-orm";
import { getDb } from "@/db/client";
import { appointments, patients, payments, recordFiles, recurringSeries, sessionRecords } from "@/db/schema";
import { getSeriesSummary } from "@/features/appointments/recurrence";
import { decryptRecordContent } from "@/lib/crypto/records";
import { AppError } from "@/lib/errors/app-error";

function isMissingRecurringSchemaError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error.code === "42703" || error.code === "42P01")
  );
}

export async function listCalendarEvents(userId: string, start: Date, end: Date) {
  return getDb()
    .select({
      id: appointments.id,
      patientId: appointments.patientId,
      title: patients.fullName,
      start: appointments.startsAt,
      end: appointments.endsAt,
      status: appointments.status,
      confirmationStatus: appointments.confirmationStatus,
    })
    .from(appointments)
    .innerJoin(patients, eq(patients.id, appointments.patientId))
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        gte(appointments.startsAt, start),
        lte(appointments.startsAt, end),
      ),
    )
    .orderBy(asc(appointments.startsAt));
}

export async function listUpcomingAppointments(userId: string) {
  const now = new Date();

  return getDb()
    .select({
      id: appointments.id,
      startsAt: appointments.startsAt,
      endsAt: appointments.endsAt,
      status: appointments.status,
      confirmationStatus: appointments.confirmationStatus,
      sessionPriceCents: appointments.sessionPriceCents,
      patientName: patients.fullName,
      patientId: patients.id,
    })
    .from(appointments)
    .innerJoin(patients, eq(patients.id, appointments.patientId))
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        gte(appointments.startsAt, now),
        eq(appointments.status, "scheduled"),
      ),
    )
    .orderBy(asc(appointments.startsAt))
    .limit(8);
}

export async function listTodaysAppointments(userId: string) {
  const start = new Date();
  start.setHours(0, 0, 0, 0);

  const end = new Date(start);
  end.setHours(23, 59, 59, 999);

  return getDb()
    .select({
      id: appointments.id,
      startsAt: appointments.startsAt,
      endsAt: appointments.endsAt,
      status: appointments.status,
      confirmationStatus: appointments.confirmationStatus,
      patientName: patients.fullName,
      patientId: patients.id,
    })
    .from(appointments)
    .innerJoin(patients, eq(patients.id, appointments.patientId))
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        gte(appointments.startsAt, start),
        lte(appointments.startsAt, end),
      ),
    )
    .orderBy(asc(appointments.startsAt));
}

export async function getAppointmentDetail(userId: string, appointmentId: string) {
  const db = getDb();

  let appointment:
    | {
        id: string;
        patientId: string;
        patientName: string;
        startsAt: Date;
        endsAt: Date;
        seriesId: string | null;
        status: "scheduled" | "completed" | "cancelled" | "no_show";
        confirmationStatus: "unconfirmed" | "confirmed" | "cancelled";
        recurringFrequency: "weekly" | "biweekly" | null;
        recurringStartsOn: string | null;
        recurringEndsOn: string | null;
        recurringOccurrencesCount: number | null;
        recurringStartTime: string | null;
        recurringEndTime: string | null;
        recurringCancelledAt: Date | null;
        sessionPriceCents: number;
        quickNotes: string | null;
        cancelReason: string | null;
        paymentId: string | null;
        paymentStatus: "pending" | "paid" | "cancelled" | null;
        paymentMethod: "pix" | "cash" | "card" | "bank_transfer" | "other" | null;
        amountReceivedCents: number | null;
        paidAt: Date | null;
        paymentNotes: string | null;
        recordId: string | null;
        encryptedPayload: string | null;
        iv: string | null;
        authTag: string | null;
        keyVersion: number | null;
      }
    | undefined;

  try {
    [appointment] = await db
      .select({
        id: appointments.id,
        patientId: appointments.patientId,
        patientName: patients.fullName,
        startsAt: appointments.startsAt,
        endsAt: appointments.endsAt,
        seriesId: appointments.seriesId,
        status: appointments.status,
        confirmationStatus: appointments.confirmationStatus,
        recurringFrequency: recurringSeries.frequency,
        recurringStartsOn: recurringSeries.startsOn,
        recurringEndsOn: recurringSeries.endsOn,
        recurringOccurrencesCount: recurringSeries.occurrencesCount,
        recurringStartTime: recurringSeries.startTime,
        recurringEndTime: recurringSeries.endTime,
        recurringCancelledAt: recurringSeries.cancelledAt,
        sessionPriceCents: appointments.sessionPriceCents,
        quickNotes: appointments.quickNotes,
        cancelReason: appointments.cancelReason,
        paymentId: payments.id,
        paymentStatus: payments.status,
        paymentMethod: payments.method,
        amountReceivedCents: payments.amountReceivedCents,
        paidAt: payments.paidAt,
        paymentNotes: payments.notes,
        recordId: sessionRecords.id,
        encryptedPayload: sessionRecords.encryptedPayload,
        iv: sessionRecords.iv,
        authTag: sessionRecords.authTag,
        keyVersion: sessionRecords.keyVersion,
      })
      .from(appointments)
      .innerJoin(patients, eq(patients.id, appointments.patientId))
      .leftJoin(recurringSeries, eq(recurringSeries.id, appointments.seriesId))
      .leftJoin(
        payments,
        and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)),
      )
      .leftJoin(
        sessionRecords,
        and(eq(sessionRecords.appointmentId, appointments.id), isNull(sessionRecords.deletedAt)),
      )
      .where(and(eq(appointments.userId, userId), eq(appointments.id, appointmentId), isNull(appointments.deletedAt)));
  } catch (error) {
    if (!isMissingRecurringSchemaError(error)) {
      throw error;
    }

    [appointment] = await db
      .select({
        id: appointments.id,
        patientId: appointments.patientId,
        patientName: patients.fullName,
        startsAt: appointments.startsAt,
        endsAt: appointments.endsAt,
        status: appointments.status,
        confirmationStatus: appointments.confirmationStatus,
        sessionPriceCents: appointments.sessionPriceCents,
        quickNotes: appointments.quickNotes,
        cancelReason: appointments.cancelReason,
        paymentId: payments.id,
        paymentStatus: payments.status,
        paymentMethod: payments.method,
        amountReceivedCents: payments.amountReceivedCents,
        paidAt: payments.paidAt,
        paymentNotes: payments.notes,
        recordId: sessionRecords.id,
        encryptedPayload: sessionRecords.encryptedPayload,
        iv: sessionRecords.iv,
        authTag: sessionRecords.authTag,
        keyVersion: sessionRecords.keyVersion,
      })
      .from(appointments)
      .innerJoin(patients, eq(patients.id, appointments.patientId))
      .leftJoin(
        payments,
        and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)),
      )
      .leftJoin(
        sessionRecords,
        and(eq(sessionRecords.appointmentId, appointments.id), isNull(sessionRecords.deletedAt)),
      )
      .where(and(eq(appointments.userId, userId), eq(appointments.id, appointmentId), isNull(appointments.deletedAt)))
      .then((rows) =>
        rows.map((row) => ({
          ...row,
          seriesId: null,
          recurringFrequency: null,
          recurringStartsOn: null,
          recurringEndsOn: null,
          recurringOccurrencesCount: null,
          recurringStartTime: null,
          recurringEndTime: null,
          recurringCancelledAt: null,
        })),
      );
  }

  if (!appointment) {
    throw new AppError("Atendimento nao encontrado.", {
      statusCode: 404,
      code: "APPOINTMENT_NOT_FOUND",
    });
  }

  const files = await db
    .select({
      id: recordFiles.id,
      originalName: recordFiles.originalName,
      mimeType: recordFiles.mimeType,
      byteSize: recordFiles.byteSize,
      uploadedAt: recordFiles.uploadedAt,
      kind: recordFiles.kind,
      paymentId: recordFiles.paymentId,
    })
    .from(recordFiles)
    .where(
      and(
        eq(recordFiles.userId, userId),
        eq(recordFiles.appointmentId, appointmentId),
        isNull(recordFiles.deletedAt),
      ),
    )
    .orderBy(desc(recordFiles.uploadedAt));

  return {
    ...appointment,
    recurringSeries:
      appointment.seriesId && appointment.recurringFrequency
        ? {
            id: appointment.seriesId,
            frequency: appointment.recurringFrequency,
            startsOn: appointment.recurringStartsOn,
            endsOn: appointment.recurringEndsOn,
            occurrencesCount: appointment.recurringOccurrencesCount,
            startTime: appointment.recurringStartTime,
            endTime: appointment.recurringEndTime,
            cancelledAt: appointment.recurringCancelledAt,
            summary: getSeriesSummary({
              frequency: appointment.recurringFrequency,
              startsOn: appointment.recurringStartsOn!,
              endsOn: appointment.recurringEndsOn,
              occurrencesCount: appointment.recurringOccurrencesCount,
              startTime: appointment.recurringStartTime!,
              endTime: appointment.recurringEndTime!,
            }),
          }
        : null,
    recordContent:
      appointment.recordId && appointment.encryptedPayload
        ? decryptRecordContent({
            encryptedPayload: appointment.encryptedPayload,
            iv: appointment.iv!,
            authTag: appointment.authTag!,
            keyVersion: appointment.keyVersion!,
          })
        : "",
    files: files.filter((file) => file.kind === "session_attachment"),
    paymentReceipts: files.filter((file) => file.kind === "payment_receipt"),
  };
}
