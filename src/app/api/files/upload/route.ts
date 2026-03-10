import { NextResponse } from "next/server";
import { confirmUploadSchema, fileUploadRequestSchema } from "@/features/files/schemas";
import { confirmUpload, createUploadSession } from "@/features/files/service";
import { getStorageBucket } from "@/lib/env";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  try {
    const user = await getCurrentUser();

    if (!user) {
      return NextResponse.json({ message: "Nao autenticado." }, { status: 401 });
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
      return NextResponse.json(
        {
          message: "Dados de upload invalidos.",
          errors: parsed.error.flatten().fieldErrors,
        },
        { status: 400 },
      );
    }

    const session = await createUploadSession(user.id, parsed.data);

    return NextResponse.json({
      ...session,
      bucket: getStorageBucket(),
    });
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao iniciar upload.";
    const status = error instanceof AppError ? error.statusCode : 500;
    return NextResponse.json({ message }, { status });
  }
}

export async function PATCH(request: Request) {
  try {
    const user = await getCurrentUser();

    if (!user) {
      return NextResponse.json({ message: "Nao autenticado." }, { status: 401 });
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
      return NextResponse.json({ message: "Arquivo invalido." }, { status: 400 });
    }

    const file = await confirmUpload(user.id, parsed.data.fileId);
    return NextResponse.json({ success: true, file });
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao confirmar upload.";
    const status = error instanceof AppError ? error.statusCode : 500;
    return NextResponse.json({ message }, { status });
  }
}
