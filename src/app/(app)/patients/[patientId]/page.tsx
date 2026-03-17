import { Download, MessageCircle, Video } from "lucide-react";
import { PatientAdminInlineEditor } from "@/components/forms/patient-admin-inline-editor";
import { AppointmentForm } from "@/components/forms/appointment-form";
import { PageHeader } from "@/components/shell/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cancelRecurringSeriesAction } from "@/features/appointments/actions";
import { deactivatePatientAction, reactivatePatientAction } from "@/features/patients/actions";
import { getPatientDetail } from "@/features/patients/queries";
import { requireUser } from "@/lib/auth/session";
import { PatientTimelineClient } from "./_components/patient-timeline-client";
import {
  buildGoogleMeetUrl,
  buildWhatsAppUrl,
  getPatientStatusLabel,
} from "@/lib/utils";

type PatientDetailPageProps = {
  params: Promise<{ patientId: string }>;
};

export default async function PatientDetailPage({ params }: PatientDetailPageProps) {
  const user = await requireUser();
  const { patientId } = await params;
  const { patient, timeline, recurringSeries } = await getPatientDetail(user.id, patientId);
  const whatsappUrl = buildWhatsAppUrl(patient.phone);
  const meetUrl = buildGoogleMeetUrl();

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Ficha do paciente"
        title={patient.fullName}
        description="Dados administrativos, historico de atendimentos, registros e anexos em um unico fluxo de trabalho."
        actions={
          <div className="flex flex-wrap gap-3">
            <Button asChild variant="outline">
              <a href={`/api/patients/${patient.id}/export`}>
                <Download className="h-4 w-4" />
                Exportar dados
              </a>
            </Button>
            <form
              action={async () => {
                "use server";
                if (patient.status === "active") {
                  await deactivatePatientAction(patient.id);
                  return;
                }

                await reactivatePatientAction(patient.id);
              }}
            >
              <Button type="submit" variant="outline">
                {patient.status === "active" ? "Desativar paciente" : "Reativar paciente"}
              </Button>
            </form>
          </div>
        }
      />

      <div className="flex flex-wrap gap-2">
        {patient.chartNumber ? (
          <Badge className="rounded-full px-4 py-1 text-xs uppercase tracking-[0.16em]" variant="outline">
            Prontuario: {patient.chartNumber}
          </Badge>
        ) : null}
        <Badge className="rounded-full px-4 py-1 text-xs uppercase tracking-[0.16em]" variant={patient.status === "active" ? "success" : "secondary"}>
          {getPatientStatusLabel(patient.status)}
        </Badge>
      </div>

      <div className="grid gap-6 xl:grid-cols-[0.95fr_1.2fr]">
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <CardTitle>Dados administrativos</CardTitle>
                  <CardDescription>Informacoes centrais para contato e organizacao da rotina.</CardDescription>
                </div>
                <Badge variant={patient.status === "active" ? "success" : "secondary"}>
                  {getPatientStatusLabel(patient.status)}
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="space-y-5">
              <div className="flex flex-wrap gap-3">
                {whatsappUrl ? (
                  <Button asChild variant="outline">
                    <a href={whatsappUrl} rel="noreferrer" target="_blank">
                      <MessageCircle className="h-4 w-4" />
                      WhatsApp
                    </a>
                  </Button>
                ) : null}
                <Button asChild variant="outline">
                  <a href={meetUrl} rel="noreferrer" target="_blank">
                    <Video className="h-4 w-4" />
                    Nova chamada
                  </a>
                </Button>
              </div>

              <PatientAdminInlineEditor
                patientId={patient.id}
                values={{
                  adminNotes: patient.adminNotes ?? "",
                  birthDate: patient.birthDate ?? "",
                  chartNumber: patient.chartNumber ?? "",
                  email: patient.email ?? "",
                  emergencyPhone: patient.emergencyPhone ?? "",
                  fullName: patient.fullName,
                  healthHistory: patient.healthHistory ?? "",
                  medicationsInUse: patient.medicationsInUse ?? "",
                  phone: patient.phone ?? "",
                }}
              />

              <details
                id="novo-agendamento"
                className="group rounded-[28px] border border-primary/20 bg-primary/5 p-4 shadow-[0_18px_40px_-28px_rgba(16,89,98,0.55)]"
              >
                <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-full bg-white px-5 py-3 text-sm font-semibold text-slate-900 transition hover:bg-primary/10">
                  <span>Novo agendamento para este paciente</span>
                </summary>
                <div className="mt-4">
                  <AppointmentForm
                    defaultValues={{
                      patientId: patient.id,
                      sessionPriceCents: "180,00",
                      status: "scheduled",
                      confirmationStatus: "unconfirmed",
                    }}
                    patientOptions={[{ id: patient.id, fullName: patient.fullName }]}
                  />
                </div>
              </details>
            </CardContent>
          </Card>

        </div>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Rotina recorrente</CardTitle>
              <CardDescription>Series ativas e encerradas para este paciente.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {recurringSeries.length ? (
                recurringSeries.map((series) => (
                  <div key={series.id} className="rounded-[28px] border border-border/80 bg-white p-5">
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="space-y-2">
                        <p className="font-semibold text-slate-900">{series.summary}</p>
                        <Badge variant={series.cancelledAt ? "secondary" : "success"}>
                          {series.cancelledAt ? "Serie encerrada" : "Serie ativa"}
                        </Badge>
                      </div>
                      {!series.cancelledAt ? (
                        <form
                          action={async () => {
                            "use server";
                            await cancelRecurringSeriesAction(series.id);
                          }}
                        >
                          <Button type="submit" variant="outline">
                            Encerrar serie
                          </Button>
                        </form>
                      ) : null}
                    </div>
                  </div>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">Nenhuma serie recorrente configurada para este paciente.</p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Linha do tempo de atendimentos</CardTitle>
              <CardDescription>Resumo textual rapido, status financeiro e anexos por atendimento.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <PatientTimelineClient patientId={patient.id} timeline={timeline} />
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
