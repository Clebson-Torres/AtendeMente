import { getDb } from "@/db/client";
import { patients } from "@/db/schema";
import { and, eq, isNull } from "drizzle-orm";
import {
  detectExistingPatientDuplicates,
  detectImportedPatientDuplicates,
  parsePatientsSpreadsheet,
} from "@/features/patients/import";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { logError, logInfo, logWarn } from "@/lib/observability/logger";
import { buildRequestLogContext, jsonWithRequestId } from "@/lib/observability/request";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  const startedAt = Date.now();

  try {
    const user = await getCurrentUser();
    const logContext = buildRequestLogContext(request, "/api/patients/import/preview", {
      userId: user?.id ?? null,
    });

    if (!user) {
      logWarn("patients.import_preview.unauthenticated", logContext);
      return jsonWithRequestId(request, { message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "patients:import:preview",
      identifier: user.id,
      limit: 10,
      windowMs: 15 * 60 * 1000,
    });

    const formData = await request.formData();
    const file = formData.get("file");

    if (!(file instanceof File)) {
      logWarn("patients.import_preview.invalid_file", {
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

    logInfo("patients.import_preview.success", {
      ...logContext,
      durationMs: Date.now() - startedAt,
      fileName: file.name,
      totalRows: preview.rows.length + preview.errors.length,
      validRows: preview.rows.length,
      invalidRows: preview.errors.length,
      duplicateRows: duplicates.length + existingDuplicates.length,
    });

    return jsonWithRequestId(
      request,
      {
        rows: preview.rows,
        errors: preview.errors,
        duplicates,
        existingDuplicates,
        summary: {
          totalRows: preview.rows.length + preview.errors.length,
          validRows: preview.rows.length,
          invalidRows: preview.errors.length,
          duplicateRows: duplicates.length + existingDuplicates.length,
        },
      },
      { status: 200 },
    );
  } catch (error) {
    if (error instanceof AppError) {
      logWarn("patients.import_preview.app_error", {
        ...buildRequestLogContext(request, "/api/patients/import/preview"),
        durationMs: Date.now() - startedAt,
        statusCode: error.statusCode,
        errorCode: error.code,
      });
      return jsonWithRequestId(request, { message: error.message }, { status: error.statusCode });
    }

    const databaseError = error as { code?: string };

    if (databaseError?.code === "42703" || databaseError?.code === "42P01") {
      logError("patients.import_preview.database_schema_error", error, {
        ...buildRequestLogContext(request, "/api/patients/import/preview"),
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

    logError("patients.import_preview.failed", error, {
      ...buildRequestLogContext(request, "/api/patients/import/preview"),
      durationMs: Date.now() - startedAt,
    });
    return jsonWithRequestId(
      request,
      {
        message: "Nao foi possivel analisar o arquivo agora. Tente novamente em instantes.",
      },
      { status: 500 },
    );
  }
}
