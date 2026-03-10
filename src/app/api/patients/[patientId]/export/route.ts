import { exportPatientBundle } from "@/features/exports/service";
import { getCurrentUser } from "@/lib/auth/session";
import { AppError } from "@/lib/errors/app-error";
import { enforceRateLimit } from "@/lib/security/rate-limit";

export const runtime = "nodejs";

type ExportRouteProps = {
  params: Promise<{ patientId: string }>;
};

export async function GET(_: Request, { params }: ExportRouteProps) {
  try {
    const user = await getCurrentUser();

    if (!user) {
      return Response.json({ message: "Nao autenticado." }, { status: 401 });
    }

    await enforceRateLimit({
      scope: "patients:export",
      identifier: user.id,
      limit: 10,
      windowMs: 60 * 60 * 1000,
    });

    const { patientId } = await params;
    const exported = await exportPatientBundle(user.id, patientId);

    return new Response(new Uint8Array(exported.buffer), {
      headers: {
        "Content-Type": "application/zip",
        "Content-Disposition": `attachment; filename="patient-${exported.patient.id}.zip"`,
      },
    });
  } catch (error) {
    const message = error instanceof AppError ? error.message : "Falha ao exportar paciente.";
    const status = error instanceof AppError ? error.statusCode : 500;
    return Response.json({ message }, { status });
  }
}
