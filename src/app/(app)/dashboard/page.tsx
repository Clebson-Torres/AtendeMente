import Link from "next/link";
import { ArrowRight, CalendarClock } from "lucide-react";
import { getDashboardData } from "@/features/dashboard/queries";
import { requireUser } from "@/lib/auth/session";
import {
  describeAppointmentTime,
  getAppointmentConfirmationBadgeVariant,
  getAppointmentConfirmationLabel,
  getAppointmentStatusBadgeVariant,
  getAppointmentStatusLabel,
} from "@/lib/utils";
import { EmptyState } from "@/components/shell/empty-state";
import { MetricCard } from "@/components/shell/metric-card";
import { PageHeader } from "@/components/shell/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export default async function DashboardPage() {
  const user = await requireUser();
  const data = await getDashboardData(user.id);

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow="Visao geral"
        title="Comece pelo que acontece hoje"
        description="Priorize os atendimentos do dia, veja o que vem a seguir e entre na agenda com menos atrito."
      />

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard
          hint="Quantidade de compromissos marcados para hoje."
          label="Atendimentos de hoje"
          value={String(data.todaysAppointments.length)}
        />
        <MetricCard
          hint="Compromissos futuros ativos na agenda."
          label="Proximos na agenda"
          value={String(data.upcomingAppointments.length)}
        />
        <MetricCard
          hint="Atendimentos ativos cadastrados a partir do ciclo atual."
          label="Atendimentos no ciclo"
          value={String(data.financialSummary.appointmentsCount)}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.25fr_0.95fr]">
        <Card>
          <CardHeader className="flex flex-row items-end justify-between">
            <div className="space-y-2">
              <CardTitle>Atendimentos de hoje</CardTitle>
              <CardDescription>Veja primeiro quem sera atendido hoje e em qual horario.</CardDescription>
            </div>
            <Button asChild variant="outline">
              <Link href="/agenda">
                Ver agenda
                <ArrowRight className="h-4 w-4" />
              </Link>
            </Button>
          </CardHeader>
          <CardContent className="space-y-4">
            {data.todaysAppointments.length ? (
              data.todaysAppointments.map((appointment) => (
                <Link
                  key={appointment.id}
                  className="block rounded-3xl border border-border/70 bg-white p-4 transition hover:border-primary/30 hover:bg-muted/40"
                  href={`/appointments/${appointment.id}`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="space-y-2">
                      <p className="font-semibold text-slate-900">{appointment.patientName}</p>
                      <p className="text-sm text-muted-foreground">{describeAppointmentTime(appointment.startsAt)}</p>
                      <div className="flex flex-wrap gap-2">
                        <Badge variant={getAppointmentStatusBadgeVariant(appointment.status)}>
                          {getAppointmentStatusLabel(appointment.status)}
                        </Badge>
                        <Badge variant={getAppointmentConfirmationBadgeVariant(appointment.confirmationStatus)}>
                          {getAppointmentConfirmationLabel(appointment.confirmationStatus)}
                        </Badge>
                      </div>
                    </div>
                    <CalendarClock className="h-5 w-5 text-primary" />
                  </div>
                </Link>
              ))
            ) : (
              <EmptyState
                title="Nenhum atendimento hoje"
                description="Assim que voce cadastrar compromissos para o dia, eles aparecerao aqui."
              />
            )}
          </CardContent>
        </Card>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Proximos atendimentos</CardTitle>
              <CardDescription>Os compromissos mais proximos para voce se preparar com antecedencia.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {data.upcomingAppointments.length ? (
                data.upcomingAppointments.map((appointment) => (
                  <Link
                    key={appointment.id}
                    className="flex items-center justify-between rounded-3xl bg-muted/40 p-4 transition hover:bg-muted/60"
                    href={`/appointments/${appointment.id}`}
                  >
                    <div className="space-y-1">
                      <p className="font-semibold">{appointment.patientName}</p>
                      <p className="text-sm text-muted-foreground">{describeAppointmentTime(appointment.startsAt)}</p>
                    </div>
                    <Badge variant={appointment.status === "scheduled" ? "default" : "secondary"}>
                      {getAppointmentStatusLabel(appointment.status)}
                    </Badge>
                  </Link>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">Nenhum atendimento futuro na agenda.</p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Atalhos rapidos</CardTitle>
              <CardDescription>Navegue para os fluxos mais usados na rotina sem misturar assuntos financeiros.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-3 sm:grid-cols-2">
              <Link
                className="rounded-3xl border border-border/80 bg-muted/30 p-4 transition hover:border-primary/30 hover:bg-primary/5"
                href="/agenda"
              >
                <p className="font-semibold text-slate-900">Abrir agenda</p>
                <p className="mt-1 text-sm text-muted-foreground">Selecione um horario e cadastre o atendimento sem sair da visao do dia.</p>
              </Link>
              <Link
                className="rounded-3xl border border-border/80 bg-muted/30 p-4 transition hover:border-primary/30 hover:bg-primary/5"
                href="/patients"
              >
                <p className="font-semibold text-slate-900">Ver pacientes</p>
                <p className="mt-1 text-sm text-muted-foreground">Abra fichas, revise historico e atualize dados administrativos.</p>
              </Link>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
