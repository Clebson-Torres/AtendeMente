import { NextResponse } from "next/server";
import { and, eq, isNull } from "drizzle-orm";
import { getDb } from "@/db/client";
import { patients } from "@/db/schema";
import {
  detectExistingPatientDuplicates,
  detectImportedPatientDuplicates,
  parsePatientsSpreadsheet,
} from "@/features/patients/import";
import { writeAuditLog } from "@/lib/audit/log";
import { getCurrentUser } from "@/lib/auth/session";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  try {
    const user = await getCurrentUser();

    if (!user) {
      return NextResponse.json({ message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "patients:import:commit",
      identifier: user.id,
      limit: 5,
      windowMs: 15 * 60 * 1000,
    });

    const formData = await request.formData();
    const file = formData.get("file");

    if (!(file instanceof File)) {
      return NextResponse.json({ message: "Arquivo invalido." }, { status: 400 });
    }

    const preview = await parsePatientsSpreadsheet(file);
    const duplicates = detectImportedPatientDuplicates(preview.rows);
    const existingPatients = await getDb()
      .select({
        id: patients.id,
        fullName: patients.fullName,
        phone: patients.phone,
      })
      .from(patients)
      .where(and(eq(patients.userId, user.id), isNull(patients.deletedAt)));
    const existingDuplicates = detectExistingPatientDuplicates(preview.rows, existingPatients);

    if (preview.errors.length) {
      return NextResponse.json({ message: "Corrija as linhas invalidas antes de importar.", errors: preview.errors }, { status: 400 });
    }

    const rowsToInsert = preview.rows.filter(
      (row) =>
        !duplicates.some((duplicate) => duplicate.row === row.rowNumber) &&
        !existingDuplicates.some((duplicate) => duplicate.row === row.rowNumber),
    );

    if (!rowsToInsert.length) {
      return NextResponse.json({ message: "Nenhuma linha valida para importar." }, { status: 400 });
    }

    await getDb().insert(patients).values(
      rowsToInsert.map((row) => ({
        userId: user.id,
        fullName: row.fullName,
        phone: row.phone || null,
        email: row.email || null,
        birthDate: row.birthDate || null,
        emergencyPhone: row.emergencyPhone || null,
        medicationsInUse: row.medicationsInUse || null,
        healthHistory: row.healthHistory || null,
        adminNotes: row.adminNotes || null,
      })),
    );

    await writeAuditLog({
      userId: user.id,
      action: "update",
      entityType: "patient_import",
      metadata: {
        importedCount: rowsToInsert.length,
        duplicateRows: duplicates.length + existingDuplicates.length,
        sourceFile: file.name,
      },
    });

    return NextResponse.json({
      success: true,
      importedCount: rowsToInsert.length,
      duplicateRows: duplicates.length + existingDuplicates.length,
    });
  } catch (error) {
    const databaseError = error as { code?: string; message?: string };

    if (databaseError?.code === "42703" || databaseError?.code === "42P01") {
      return NextResponse.json(
        {
          message:
            "O banco da aplicacao publicada ainda nao esta atualizado para a importacao. Aplique as migrations mais recentes no Supabase e tente novamente.",
        },
        { status: 500 },
      );
    }

    return NextResponse.json(
      {
        message: "Nao foi possivel concluir a importacao agora. Tente novamente em instantes.",
      },
      { status: 500 },
    );
  }
}
