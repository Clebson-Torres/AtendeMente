import { NextResponse } from "next/server";
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
import { enforceRateLimit } from "@/lib/security/rate-limit";

export async function POST(request: Request) {
  try {
    const user = await getCurrentUser();

    if (!user) {
      return NextResponse.json({ message: "Nao autenticado." }, { status: 401 });
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

    return NextResponse.json({
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
    });
  } catch (error) {
    if (error instanceof AppError) {
      return NextResponse.json({ message: error.message }, { status: error.statusCode });
    }

    const databaseError = error as { code?: string };

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
        message: "Nao foi possivel analisar o arquivo agora. Tente novamente em instantes.",
      },
      { status: 500 },
    );
  }
}
