"use server";

import { and, eq, gte, gt, isNull, lt, ne } from "drizzle-orm";
import { revalidatePath } from "next/cache";
import { getDb } from "@/db/client";
import { appointments, recurringSeries } from "@/db/schema";
import { writeAuditLog } from "@/lib/audit/log";
import { requireUser } from "@/lib/auth/session";
import { buildRecurringAppointments } from "@/features/appointments/recurrence";
import { appointmentFormSchema } from "@/features/appointments/schemas";
import type { ActionResponse } from "@/types/domain";

async function findDuplicateAppointment(
  userId: string,
  patientId: string,
  startsAt: Date,
  appointmentId?: string,
) {
  const startOfDay = new Date(startsAt);
  startOfDay.setHours(0, 0, 0, 0);
  const endOfDay = new Date(startOfDay);
  endOfDay.setDate(endOfDay.getDate() + 1);

  const filters = [
    eq(appointments.userId, userId),
    eq(appointments.patientId, patientId),
    isNull(appointments.deletedAt),
    ne(appointments.status, "cancelled"),
    gte(appointments.startsAt, startOfDay),
    lt(appointments.startsAt, endOfDay),
  ];

  if (appointmentId) {
    filters.push(ne(appointments.id, appointmentId));
  }

  return getDb().query.appointments.findFirst({
    where: and(...filters),
    columns: {
      id: true,
      startsAt: true,
    },
  });
}

export async function createAppointmentAction(
  input: unknown,
): Promise<ActionResponse<{ appointmentId: string; duplicateAppointmentId?: string; seriesId?: string; createdCount?: number }>> {
  const user = await requireUser();
  const parsed = appointmentFormSchema.safeParse(input);

  if (!parsed.success) {
    return {
      success: false,
      message: "Verifique os dados do atendimento.",
      errors: parsed.error.flatten().fieldErrors,
    };
  }

  const startsAt = new Date(parsed.data.startsAt);
  const endsAt = new Date(parsed.data.endsAt);
  const db = getDb();
  const isRecurring = Boolean(parsed.data.recurrenceFrequency);

  // Verificacao de conflito de agenda na hora exata
  const overlappingAppointment = await db.query.appointments.findFirst({
    where: and(
      eq(appointments.userId, user.id),
      isNull(appointments.deletedAt),
      ne(appointments.status, "cancelled"),
      lt(appointments.startsAt, endsAt),
      gt(appointments.endsAt, startsAt), // Changed from gte to gt so edge-to-edge schedules are permitted
    ),
    columns: {
      id: true,
    },
  });

  if (overlappingAppointment) {
    return {
      success: false,
      message: "Ja existe um agendamento para este horario.",
    };
  }

  if (!isRecurring) {
    const duplicate = await findDuplicateAppointment(user.id, parsed.data.patientId, startsAt);

    const [appointment] = await db
      .insert(appointments)
      .values({
        userId: user.id,
        patientId: parsed.data.patientId,
        startsAt,
        endsAt,
        status: parsed.data.status,
        confirmationStatus: parsed.data.confirmationStatus,
        sessionPriceCents: parsed.data.sessionPriceCents,
        quickNotes: parsed.data.quickNotes || null,
        cancelReason: parsed.data.cancelReason || null,
      })
      .returning({ id: appointments.id });

    await writeAuditLog({
      userId: user.id,
      action: "update",
      entityType: "appointment",
      entityId: appointment.id,
      metadata: { action: "create" },
    });

    revalidatePath("/agenda");
    revalidatePath("/dashboard");
    revalidatePath(`/patients/${parsed.data.patientId}`);

    return {
      success: true,
      message: "Atendimento criado com sucesso.",
      data: { appointmentId: appointment.id, duplicateAppointmentId: duplicate?.id, createdCount: 1 },
    };
  }

  const generatedAppointments = buildRecurringAppointments({
    startsAt,
    endsAt,
    frequency: parsed.data.recurrenceFrequency!,
    untilDate: parsed.data.recurrenceUntilDate || null,
    occurrences: parsed.data.recurrenceOccurrences ?? null,
  });

  if (generatedAppointments.length < 2) {
    return {
      success: false,
      message: "A recorrencia precisa gerar pelo menos duas sessoes.",
    };
  }

  const startDate = parsed.data.startsAt.slice(0, 10);
  const startTime = parsed.data.startsAt.slice(11, 16);
  const endTime = parsed.data.endsAt.slice(11, 16);

  const [series] = await db
    .insert(recurringSeries)
    .values({
      userId: user.id,
      patientId: parsed.data.patientId,
      frequency: parsed.data.recurrenceFrequency!,
      startsOn: startDate,
      endsOn: parsed.data.recurrenceEndMode === "until_date" ? parsed.data.recurrenceUntilDate || null : null,
      occurrencesCount: parsed.data.recurrenceEndMode === "occurrences" ? parsed.data.recurrenceOccurrences ?? null : null,
      startTime,
      endTime,
    })
    .returning({ id: recurringSeries.id });

  const duplicateAppointments = await Promise.all(
    generatedAppointments.map((item) =>
      findDuplicateAppointment(user.id, parsed.data.patientId, item.startsAt),
    ),
  );

  const rowsToInsert = generatedAppointments.map((item) => ({
    userId: user.id,
    patientId: parsed.data.patientId,
    seriesId: series.id,
    startsAt: item.startsAt,
    endsAt: item.endsAt,
    status: parsed.data.status,
    confirmationStatus: parsed.data.confirmationStatus,
    sessionPriceCents: parsed.data.sessionPriceCents,
    quickNotes: parsed.data.quickNotes || null,
    cancelReason: null,
  }));

  const insertedAppointments = await db
    .insert(appointments)
    .values(rowsToInsert)
    .returning({ id: appointments.id });

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "recurring_series",
    entityId: series.id,
    metadata: {
      action: "create",
      createdCount: insertedAppointments.length,
      duplicateAppointmentIds: duplicateAppointments.filter(Boolean).map((item) => item!.id),
    },
  });

  revalidatePath("/agenda");
  revalidatePath("/dashboard");
  revalidatePath(`/patients/${parsed.data.patientId}`);

  return {
    success: true,
    message: `Serie criada com ${insertedAppointments.length} atendimentos.`,
    data: {
      appointmentId: insertedAppointments[0]?.id ?? "",
      duplicateAppointmentId: duplicateAppointments.find(Boolean)?.id,
      seriesId: series.id,
      createdCount: insertedAppointments.length,
    },
  };
}

export async function updateAppointmentAction(
  appointmentId: string,
  input: unknown,
): Promise<ActionResponse<{ appointmentId: string; duplicateAppointmentId?: string }>> {
  const user = await requireUser();
  const parsed = appointmentFormSchema.safeParse(input);

  if (!parsed.success) {
    return {
      success: false,
      message: "Verifique os dados do atendimento.",
      errors: parsed.error.flatten().fieldErrors,
    };
  }

  const startsAt = new Date(parsed.data.startsAt);
  const endsAt = new Date(parsed.data.endsAt);

  const duplicate = await findDuplicateAppointment(user.id, parsed.data.patientId, startsAt, appointmentId);

  // Verificacao de conflito de agenda na hora exata
  const db = getDb();
  const overlappingAppointment = await db.query.appointments.findFirst({
    where: and(
      eq(appointments.userId, user.id),
      isNull(appointments.deletedAt),
      ne(appointments.status, "cancelled"),
      ne(appointments.id, appointmentId), // ignorar este que está sendo editado
      lt(appointments.startsAt, endsAt),
      gt(appointments.endsAt, startsAt), // Changed from gte to gt so edge-to-edge schedules are permitted
    ),
    columns: {
      id: true,
    },
  });

  if (overlappingAppointment) {
    return {
      success: false,
      message: "Ja existe um agendamento para este horario.",
    };
  }

  const [appointment] = await db
    .update(appointments)
    .set({
      patientId: parsed.data.patientId,
      startsAt,
      endsAt,
      status: parsed.data.status,
      confirmationStatus: parsed.data.confirmationStatus,
      sessionPriceCents: parsed.data.sessionPriceCents,
      quickNotes: parsed.data.quickNotes || null,
      cancelReason: parsed.data.cancelReason || null,
      updatedAt: new Date(),
    })
    .where(
      and(eq(appointments.id, appointmentId), eq(appointments.userId, user.id), isNull(appointments.deletedAt)),
    )
    .returning({ id: appointments.id, patientId: appointments.patientId });

  if (!appointment) {
    return {
      success: false,
      message: "Atendimento nao encontrado.",
    };
  }

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "appointment",
    entityId: appointment.id,
  });

  revalidatePath("/agenda");
  revalidatePath("/dashboard");
  revalidatePath(`/appointments/${appointment.id}`);
  revalidatePath(`/patients/${appointment.patientId}`);

  return {
    success: true,
    message: "Atendimento atualizado com sucesso.",
    data: { appointmentId: appointment.id, duplicateAppointmentId: duplicate?.id },
  };
}

export async function cancelAppointmentAction(
  appointmentId: string,
  cancelReason: string,
): Promise<ActionResponse<{ appointmentId: string }>> {
  const user = await requireUser();

  const [appointment] = await getDb()
    .update(appointments)
    .set({
      status: "cancelled",
      confirmationStatus: "cancelled",
      cancelReason: cancelReason || "Cancelado manualmente.",
      updatedAt: new Date(),
    })
    .where(
      and(eq(appointments.id, appointmentId), eq(appointments.userId, user.id), isNull(appointments.deletedAt)),
    )
    .returning({ id: appointments.id, patientId: appointments.patientId });

  if (!appointment) {
    return {
      success: false,
      message: "Atendimento nao encontrado.",
    };
  }

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "appointment",
    entityId: appointment.id,
    metadata: { action: "cancel" },
  });

  revalidatePath("/agenda");
  revalidatePath("/dashboard");
  revalidatePath(`/appointments/${appointment.id}`);
  revalidatePath(`/patients/${appointment.patientId}`);

  return {
    success: true,
    message: "Atendimento cancelado.",
    data: { appointmentId: appointment.id },
  };
}

export async function cancelRecurringSeriesAction(
  seriesId: string,
): Promise<ActionResponse<{ seriesId: string }>> {
  const user = await requireUser();
  const db = getDb();

  const [series] = await db
    .update(recurringSeries)
    .set({
      cancelledAt: new Date(),
      updatedAt: new Date(),
    })
    .where(and(eq(recurringSeries.id, seriesId), eq(recurringSeries.userId, user.id), isNull(recurringSeries.cancelledAt)))
    .returning({ id: recurringSeries.id, patientId: recurringSeries.patientId });

  if (!series) {
    return {
      success: false,
      message: "Serie recorrente nao encontrada.",
    };
  }

  const now = new Date();
  await db
    .update(appointments)
    .set({
      status: "cancelled",
      confirmationStatus: "cancelled",
      cancelReason: "Serie recorrente encerrada.",
      updatedAt: now,
    })
    .where(
      and(
        eq(appointments.userId, user.id),
        eq(appointments.seriesId, seriesId),
        gte(appointments.startsAt, now),
        isNull(appointments.deletedAt),
        ne(appointments.status, "cancelled"),
      ),
    );

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "recurring_series",
    entityId: series.id,
    metadata: { action: "cancel" },
  });

  revalidatePath("/agenda");
  revalidatePath("/dashboard");
  revalidatePath(`/patients/${series.patientId}`);

  return {
    success: true,
    message: "Serie recorrente encerrada.",
    data: { seriesId: series.id },
  };
}
