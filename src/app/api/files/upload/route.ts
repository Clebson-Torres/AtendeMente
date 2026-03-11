import { confirmUploadSchema, fileUploadRequestSchema } from "@/features/files/schemas";
import { confirmUpload, createUploadSession } from "@/features/files/service";
import { getStorageBucket } from "@/lib/env";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { logError, logInfo, logWarn } from "@/lib/observability/logger";
import { buildRequestLogContext, jsonWithRequestId } from "@/lib/observability/request";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  const startedAt = Date.now();

  try {
    const user = await getCurrentUser();
    const logContext = buildRequestLogContext(request, "/api/files/upload:init", {
      userId: user?.id ?? null,
    });

    if (!user) {
      logWarn("files.upload_init.unauthenticated", logContext);
      return jsonWithRequestId(request, { message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "files:upload:init",
      identifier: user.id,
      limit: 20,
      windowMs: 10 * 60 * 1000,
    });

    const body = await request.json();
    const parsed = fileUploadRequestSchema.safeParse(body);

    if (!parsed.success) {
      logWarn("files.upload_init.invalid_payload", {
        ...logContext,
        durationMs: Date.now() - startedAt,
      });
      return jsonWithRequestId(
        request,
        {
          message: "Dados de upload invalidos.",
          errors: parsed.error.flatten().fieldErrors,
        },
        { status: 400 },
      );
    }

    const session = await createUploadSession(user.id, parsed.data);

    logInfo("files.upload_init.success", {
      ...logContext,
      durationMs: Date.now() - startedAt,
      fileId: session.fileId,
      kind: parsed.data.kind,
    });

    return jsonWithRequestId(
      request,
      {
        ...session,
        bucket: getStorageBucket(),
      },
      { status: 200 },
    );
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao iniciar upload.";
    const status = error instanceof AppError ? error.statusCode : 500;
    logError("files.upload_init.failed", error, {
      ...buildRequestLogContext(request, "/api/files/upload:init"),
      durationMs: Date.now() - startedAt,
      statusCode: status,
    });
    return jsonWithRequestId(request, { message }, { status });
  }
}

export async function PATCH(request: Request) {
  const startedAt = Date.now();

  try {
    const user = await getCurrentUser();
    const logContext = buildRequestLogContext(request, "/api/files/upload:confirm", {
      userId: user?.id ?? null,
    });

    if (!user) {
      logWarn("files.upload_confirm.unauthenticated", logContext);
      return jsonWithRequestId(request, { message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "files:upload:confirm",
      identifier: user.id,
      limit: 30,
      windowMs: 10 * 60 * 1000,
    });

    const body = await request.json();
    const parsed = confirmUploadSchema.safeParse(body);

    if (!parsed.success) {
      logWarn("files.upload_confirm.invalid_payload", {
        ...logContext,
        durationMs: Date.now() - startedAt,
      });
      return jsonWithRequestId(request, { message: "Arquivo invalido." }, { status: 400 });
    }

    const file = await confirmUpload(user.id, parsed.data.fileId);
    logInfo("files.upload_confirm.success", {
      ...logContext,
      durationMs: Date.now() - startedAt,
      fileId: file.id,
    });
    return jsonWithRequestId(request, { success: true, file }, { status: 200 });
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao confirmar upload.";
    const status = error instanceof AppError ? error.statusCode : 500;
    logError("files.upload_confirm.failed", error, {
      ...buildRequestLogContext(request, "/api/files/upload:confirm"),
      durationMs: Date.now() - startedAt,
      statusCode: status,
    });
    return jsonWithRequestId(request, { message }, { status });
  }
}
