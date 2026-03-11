import { and, eq, isNull } from "drizzle-orm";
import { getDb } from "@/db/client";
import { patients } from "@/db/schema";
import {
  detectExistingPatientDuplicates,
  detectImportedPatientDuplicates,
  normalizeImportedBirthDate,
  parsePatientsSpreadsheet,
} from "@/features/patients/import";
import { writeAuditLog } from "@/lib/audit/log";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { logError, logInfo, logWarn } from "@/lib/observability/logger";
import { buildRequestLogContext, jsonWithRequestId } from "@/lib/observability/request";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  const startedAt = Date.now();

  try {
    const user = await getCurrentUser();
    const logContext = buildRequestLogContext(request, "/api/patients/import/commit", {
      userId: user?.id ?? null,
    });

    if (!user) {
      logWarn("patients.import_commit.unauthenticated", logContext);
      return jsonWithRequestId(request, { message: "Nao autenticado." }, { status: 401 });
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
      logWarn("patients.import_commit.invalid_file", {
        ...logContext,
        durationMs: Date.now() - startedAt,
      });
      return jsonWithRequestId(request, { message: "Arquivo invalido." }, { status: 400 });
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
      logWarn("patients.import_commit.invalid_rows", {
        ...logContext,
        durationMs: Date.now() - startedAt,
        invalidRows: preview.errors.length,
      });
      return jsonWithRequestId(
        request,
        { message: "Corrija as linhas invalidas antes de importar.", errors: preview.errors },
        { status: 400 },
      );
    }

    const rowsToInsert = preview.rows.filter(
      (row) =>
        !duplicates.some((duplicate) => duplicate.row === row.rowNumber) &&
        !existingDuplicates.some((duplicate) => duplicate.row === row.rowNumber),
    );

    if (!rowsToInsert.length) {
      logWarn("patients.import_commit.no_valid_rows", {
        ...logContext,
        durationMs: Date.now() - startedAt,
      });
      return jsonWithRequestId(request, { message: "Nenhuma linha valida para importar." }, { status: 400 });
    }

    await getDb().insert(patients).values(
      rowsToInsert.map((row) => ({
        userId: user.id,
        fullName: row.fullName,
        phone: row.phone || null,
        email: row.email || null,
        birthDate: normalizeImportedBirthDate(row.birthDate) || null,
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

    logInfo("patients.import_commit.success", {
      ...logContext,
      durationMs: Date.now() - startedAt,
      importedCount: rowsToInsert.length,
      duplicateRows: duplicates.length + existingDuplicates.length,
      fileName: file.name,
    });

    return jsonWithRequestId(
      request,
      {
        success: true,
        importedCount: rowsToInsert.length,
        duplicateRows: duplicates.length + existingDuplicates.length,
      },
      { status: 200 },
    );
  } catch (error) {
    if (error instanceof AppError) {
      logWarn("patients.import_commit.app_error", {
        ...buildRequestLogContext(request, "/api/patients/import/commit"),
        durationMs: Date.now() - startedAt,
        statusCode: error.statusCode,
        errorCode: error.code,
      });
      return jsonWithRequestId(request, { message: error.message }, { status: error.statusCode });
    }

    const databaseError = error as { code?: string; message?: string };

    if (databaseError?.code === "42703" || databaseError?.code === "42P01") {
      logError("patients.import_commit.database_schema_error", error, {
        ...buildRequestLogContext(request, "/api/patients/import/commit"),
        durationMs: Date.now() - startedAt,
      });
      return jsonWithRequestId(
        request,
        {
          message:
            "O banco da aplicacao publicada ainda nao esta atualizado para a importacao. Aplique as migrations mais recentes no Supabase e tente novamente.",
        },
        { status: 500 },
      );
    }

    logError("patients.import_commit.failed", error, {
      ...buildRequestLogContext(request, "/api/patients/import/commit"),
      durationMs: Date.now() - startedAt,
    });
    return jsonWithRequestId(
      request,
      {
        message: "Nao foi possivel concluir a importacao agora. Tente novamente em instantes.",
      },
      { status: 500 },
    );
  }
}
