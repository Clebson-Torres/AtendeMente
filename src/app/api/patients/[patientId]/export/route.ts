import { exportPatientBundle } from "@/features/exports/service";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { logError, logInfo, logWarn } from "@/lib/observability/logger";
import { buildRequestLogContext, getRequestId, jsonWithRequestId } from "@/lib/observability/request";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export const runtime = "nodejs";

type ExportRouteProps = {
  params: Promise<{ patientId: string }>;
};

export async function GET(request: Request, { params }: ExportRouteProps) {
  const startedAt = Date.now();

  try {
    const user = await getCurrentUser();
    const { patientId } = await params;
    const logContext = buildRequestLogContext(request, `/api/patients/${patientId}/export`, {
      userId: user?.id ?? null,
      patientId,
    });

    if (!user) {
      logWarn("patients.export.unauthenticated", logContext);
      return jsonWithRequestId(request, { message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "patients:export",
      identifier: user.id,
      limit: 10,
      windowMs: 60 * 60 * 1000,
    });

    const exported = await exportPatientBundle(user.id, patientId);

    logInfo("patients.export.success", {
      ...logContext,
      durationMs: Date.now() - startedAt,
      exportBytes: exported.buffer.length,
    });

    const response = new Response(new Uint8Array(exported.buffer), {
      headers: {
        "Content-Type": "application/zip",
        "Content-Disposition": `attachment; filename="patient-${exported.patient.id}.zip"`,
      },
    });

    const requestId = getRequestId(request);

    if (requestId) {
      response.headers.set("x-request-id", requestId);
    }

    return response;
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao exportar paciente.";
    const status = error instanceof AppError ? error.statusCode : 500;
    logError("patients.export.failed", error, {
      ...buildRequestLogContext(request, "/api/patients/[patientId]/export"),
      durationMs: Date.now() - startedAt,
      statusCode: status,
    });
    return jsonWithRequestId(request, { message }, { status });
  }
}
