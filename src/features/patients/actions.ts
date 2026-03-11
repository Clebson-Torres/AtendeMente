"use server";

import { and, eq, isNull, ne } from "drizzle-orm";
import { revalidatePath } from "next/cache";
import { getDb } from "@/db/client";
import { patients } from "@/db/schema";
import { writeAuditLog } from "@/lib/audit/log";
import { requireUser } from "@/lib/auth/session";
import { patientFormSchema } from "@/features/patients/schemas";
import { buildPatientIdentityKey } from "@/lib/utils";
import type { ActionResponse } from "@/types/domain";

async function findDuplicatePatient(userId: string, fullName: string, phone?: string | null, patientId?: string) {
  const db = getDb();
  const filters = [
    eq(patients.userId, userId),
    isNull(patients.deletedAt),
  ];

  if (patientId) {
    filters.push(ne(patients.id, patientId));
  }

  const candidatePatients = await db
    .select({
      id: patients.id,
      fullName: patients.fullName,
      phone: patients.phone,
    })
    .from(patients)
    .where(and(...filters));

  const inputKey = buildPatientIdentityKey({ fullName, phone });
  return candidatePatients.find((patient) => buildPatientIdentityKey(patient) === inputKey) ?? null;
}

async function findDuplicateChartNumber(userId: string, chartNumber?: string | null, patientId?: string) {
  const normalizedChartNumber = chartNumber?.trim();

  if (!normalizedChartNumber) {
    return null;
  }

  const filters = [
    eq(patients.userId, userId),
    eq(patients.chartNumber, normalizedChartNumber),
    isNull(patients.deletedAt),
  ];

  if (patientId) {
    filters.push(ne(patients.id, patientId));
  }

  const [duplicate] = await getDb()
    .select({
      id: patients.id,
      chartNumber: patients.chartNumber,
    })
    .from(patients)
    .where(and(...filters))
    .limit(1);

  return duplicate ?? null;
}

export async function createPatientAction(
  input: unknown,
): Promise<ActionResponse<{ patientId: string; duplicatePatientId?: string }>> {
  const user = await requireUser();
  const parsed = patientFormSchema.safeParse(input);

  if (!parsed.success) {
    return {
      success: false,
      message: "Verifique os dados do paciente.",
      errors: parsed.error.flatten().fieldErrors,
    };
  }

  const duplicate = await findDuplicatePatient(user.id, parsed.data.fullName, parsed.data.phone);

  if (duplicate) {
    return {
      success: false,
      message: "Ja existe um paciente com o mesmo nome e telefone na sua base.",
      data: { patientId: duplicate.id, duplicatePatientId: duplicate.id },
    };
  }

  const duplicateChartNumber = await findDuplicateChartNumber(user.id, parsed.data.chartNumber);

  if (duplicateChartNumber) {
    return {
      success: false,
      message: "Ja existe um paciente com este numero do prontuario na sua base.",
      data: { patientId: duplicateChartNumber.id, duplicatePatientId: duplicateChartNumber.id },
    };
  }

  const [patient] = await getDb()
    .insert(patients)
    .values({
      userId: user.id,
      fullName: parsed.data.fullName,
      chartNumber: parsed.data.chartNumber?.trim() || null,
      phone: parsed.data.phone || null,
      email: parsed.data.email || null,
      birthDate: parsed.data.birthDate || null,
      healthHistory: parsed.data.healthHistory || null,
      medicationsInUse: parsed.data.medicationsInUse || null,
      emergencyPhone: parsed.data.emergencyPhone || null,
      adminNotes: parsed.data.adminNotes || null,
    })
    .returning({ id: patients.id });

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "patient",
    entityId: patient.id,
    metadata: { action: "create" },
  });

  revalidatePath("/patients");
  revalidatePath("/dashboard");

  return {
    success: true,
    message: "Paciente cadastrado com sucesso.",
    data: { patientId: patient.id },
  };
}

export async function updatePatientAction(
  patientId: string,
  input: unknown,
): Promise<ActionResponse<{ patientId: string; duplicatePatientId?: string }>> {
  const user = await requireUser();
  const parsed = patientFormSchema.safeParse(input);

  if (!parsed.success) {
    return {
      success: false,
      message: "Verifique os dados do paciente.",
      errors: parsed.error.flatten().fieldErrors,
    };
  }

  const duplicate = await findDuplicatePatient(user.id, parsed.data.fullName, parsed.data.phone, patientId);

  if (duplicate) {
    return {
      success: false,
      message: "Ja existe outro paciente com o mesmo nome e telefone na sua base.",
      data: { patientId: duplicate.id, duplicatePatientId: duplicate.id },
    };
  }

  const duplicateChartNumber = await findDuplicateChartNumber(user.id, parsed.data.chartNumber, patientId);

  if (duplicateChartNumber) {
    return {
      success: false,
      message: "Ja existe um paciente com este numero do prontuario na sua base.",
      data: { patientId: duplicateChartNumber.id, duplicatePatientId: duplicateChartNumber.id },
    };
  }

  const [patient] = await getDb()
    .update(patients)
    .set({
      fullName: parsed.data.fullName,
      chartNumber: parsed.data.chartNumber?.trim() || null,
      phone: parsed.data.phone || null,
      email: parsed.data.email || null,
      birthDate: parsed.data.birthDate || null,
      healthHistory: parsed.data.healthHistory || null,
      medicationsInUse: parsed.data.medicationsInUse || null,
      emergencyPhone: parsed.data.emergencyPhone || null,
      adminNotes: parsed.data.adminNotes || null,
      updatedAt: new Date(),
    })
    .where(and(eq(patients.id, patientId), eq(patients.userId, user.id), isNull(patients.deletedAt)))
    .returning({ id: patients.id });

  if (!patient) {
    return {
      success: false,
      message: "Paciente nao encontrado.",
    };
  }

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "patient",
    entityId: patient.id,
  });

  revalidatePath("/patients");
  revalidatePath(`/patients/${patient.id}`);

  return {
    success: true,
    message: "Paciente atualizado com sucesso.",
    data: { patientId: patient.id },
  };
}

export async function deactivatePatientAction(patientId: string): Promise<ActionResponse<{ patientId: string }>> {
  const user = await requireUser();

  const [patient] = await getDb()
    .update(patients)
    .set({
      status: "inactive",
      updatedAt: new Date(),
    })
    .where(and(eq(patients.id, patientId), eq(patients.userId, user.id), isNull(patients.deletedAt)))
    .returning({ id: patients.id });

  if (!patient) {
    return {
      success: false,
      message: "Paciente nao encontrado.",
    };
  }

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "patient",
    entityId: patient.id,
    metadata: { action: "deactivate" },
  });

  revalidatePath("/patients");
  revalidatePath(`/patients/${patient.id}`);

  return {
    success: true,
    message: "Paciente desativado com sucesso.",
    data: { patientId: patient.id },
  };
}

export async function reactivatePatientAction(patientId: string): Promise<ActionResponse<{ patientId: string }>> {
  const user = await requireUser();

  const [patient] = await getDb()
    .update(patients)
    .set({
      status: "active",
      updatedAt: new Date(),
    })
    .where(and(eq(patients.id, patientId), eq(patients.userId, user.id), isNull(patients.deletedAt)))
    .returning({ id: patients.id });

  if (!patient) {
    return {
      success: false,
      message: "Paciente nao encontrado.",
    };
  }

  await writeAuditLog({
    userId: user.id,
    action: "update",
    entityType: "patient",
    entityId: patient.id,
    metadata: { action: "reactivate" },
  });

  revalidatePath("/patients");
  revalidatePath(`/patients/${patient.id}`);

  return {
    success: true,
    message: "Paciente reativado com sucesso.",
    data: { patientId: patient.id },
  };
}
