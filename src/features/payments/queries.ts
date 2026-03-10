import "server-only";
import { and, desc, eq, gte, isNull, lte, ne, or, sql } from "drizzle-orm";
import { getDb } from "@/db/client";
import { appointments, patients, payments } from "@/db/schema";
import type { AppointmentStatus, PaymentMethod, PaymentStatus } from "@/types/domain";

function mapPaymentRow<
  T extends {
    paymentId: string | null;
    appointmentId: string;
    appointmentStatus: AppointmentStatus;
    status: PaymentStatus | null;
    method: PaymentMethod | null;
    amountReceivedCents: number | null;
  },
>(row: T) {
  return {
    ...row,
    id: row.paymentId ?? row.appointmentId,
    status: row.status ?? "pending",
    method: row.method ?? "other",
    amountReceivedCents: row.amountReceivedCents ?? 0,
  };
}

export async function getFinancialSummary(userId: string) {
  const db = getDb();
  const monthStart = new Date();
  monthStart.setDate(1);
  monthStart.setHours(0, 0, 0, 0);

  const monthEnd = new Date(monthStart);
  monthEnd.setMonth(monthEnd.getMonth() + 1);
  monthEnd.setMilliseconds(-1);

  const [summary] = await db
    .select({
      paidCents:
        sql<number>`coalesce(sum(case when ${payments.status} = 'paid' then ${payments.amountReceivedCents} else 0 end), 0)`.mapWith(
          Number,
        ),
      pendingCents:
        sql<number>`coalesce(sum(case when ${appointments.status} = 'completed' and (${payments.id} is null or ${payments.status} = 'pending') then ${appointments.sessionPriceCents} else 0 end), 0)`.mapWith(
          Number,
        ),
    })
    .from(appointments)
    .leftJoin(payments, and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)))
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        ne(appointments.status, "cancelled"),
        gte(appointments.startsAt, monthStart),
        lte(appointments.startsAt, monthEnd),
      ),
    );

  return {
    paidCents: summary?.paidCents ?? 0,
    pendingCents: summary?.pendingCents ?? 0,
  };
}

export async function listPendingPayments(userId: string) {
  const rows = await getDb()
    .select({
      paymentId: payments.id,
      appointmentId: appointments.id,
      appointmentStatus: appointments.status,
      status: payments.status,
      method: payments.method,
      amountReceivedCents: payments.amountReceivedCents,
      paidAt: payments.paidAt,
      patientName: patients.fullName,
      startsAt: appointments.startsAt,
      sessionPriceCents: appointments.sessionPriceCents,
    })
    .from(appointments)
    .leftJoin(payments, and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)))
    .innerJoin(patients, eq(patients.id, appointments.patientId))
    .where(
      and(
        eq(appointments.userId, userId),
        isNull(appointments.deletedAt),
        eq(appointments.status, "completed"),
        or(isNull(payments.id), eq(payments.status, "pending")),
      ),
    )
    .orderBy(desc(appointments.startsAt));

  return rows.map(mapPaymentRow);
}

export async function listPayments(userId: string) {
  const rows = await getDb()
    .select({
      paymentId: payments.id,
      appointmentId: appointments.id,
      appointmentStatus: appointments.status,
      status: payments.status,
      method: payments.method,
      amountReceivedCents: payments.amountReceivedCents,
      paidAt: payments.paidAt,
      patientName: patients.fullName,
      startsAt: appointments.startsAt,
      sessionPriceCents: appointments.sessionPriceCents,
    })
    .from(appointments)
    .leftJoin(payments, and(eq(payments.appointmentId, appointments.id), isNull(payments.deletedAt)))
    .innerJoin(patients, eq(patients.id, appointments.patientId))
    .where(and(eq(appointments.userId, userId), isNull(appointments.deletedAt), ne(appointments.status, "cancelled")))
    .orderBy(desc(appointments.startsAt));

  return rows.map(mapPaymentRow);
}
