"use client";

import { useState } from "react";
import Link from "next/link";
import { FileText, CalendarClock } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/shell/empty-state";
import { RescheduleDialog } from "@/components/forms/reschedule-dialog";
import {
  describeAppointmentTime,
  formatCurrencyBRL,
  getAppointmentConfirmationBadgeVariant,
  getAppointmentConfirmationLabel,
  getAppointmentStatusBadgeVariant,
  getAppointmentStatusLabel,
  getPaymentStatusLabel,
  getRecordFileKindLabel,
} from "@/lib/utils";
import { type RecordFileKind } from "@/types/domain";

type TimelineItem = {
  appointmentId: string;
  startsAt: Date;
  endsAt: Date;
  status: "scheduled" | "completed" | "cancelled" | "no_show";
  confirmationStatus: "unconfirmed" | "confirmed" | "cancelled";
  paymentStatus: "pending" | "paid" | "cancelled" | null;
  paymentMethod: "pix" | "cash" | "card" | "bank_transfer" | "other" | null;
  amountReceivedCents: number | null;
  paidAt: Date | null;
  sessionPriceCents: number;
  seriesId: string | null;
  summary: string | null;
  quickNotes: string | null;
  files: Array<{
    id: string;
    originalName: string;
    kind: RecordFileKind;
  }>;
};

type PatientTimelineClientProps = {
  patientId: string;
  timeline: TimelineItem[];
};

export function PatientTimelineClient({ patientId, timeline }: PatientTimelineClientProps) {
  const [rescheduleData, setRescheduleData] = useState<{ id: string; startsAt: string; endsAt: string } | null>(null);

  if (!timeline.length) {
    return (
      <EmptyState
        title="Sem atendimentos registrados"
        description="Assim que os atendimentos forem criados, a linha do tempo completa aparecera aqui."
      />
    );
  }

  return (
    <>
      <RescheduleDialog 
        appointmentId={rescheduleData?.id || null}
        patientId={patientId}
        currentStartsAt={rescheduleData?.startsAt || ""}
        currentEndsAt={rescheduleData?.endsAt || ""}
        onOpenChange={(open) => !open && setRescheduleData(null)}
      />

      {timeline.map((item) => (
        <div key={item.appointmentId} className="rounded-[28px] border border-border/80 bg-white p-5">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="space-y-1">
              <div className="flex items-center gap-3">
                <Link className="text-lg font-semibold text-slate-900" href={`/appointments/${item.appointmentId}`}>
                  {describeAppointmentTime(item.startsAt)}
                </Link>
                {item.status === 'scheduled' && (
                  <Button 
                    variant="outline" 
                    size="sm" 
                    className="h-7 text-xs px-2 gap-1.5"
                    onClick={() => {
                      setRescheduleData({
                         id: item.appointmentId,
                         startsAt: item.startsAt.toISOString(),
                         endsAt: item.endsAt.toISOString(),
                      });
                    }}
                  >
                    <CalendarClock className="w-3.5 h-3.5" />
                    Reagendar
                  </Button>
                )}
              </div>

              <div className="flex flex-wrap gap-2 pt-1">
                <Badge variant={getAppointmentStatusBadgeVariant(item.status)}>
                  {getAppointmentStatusLabel(item.status)}
                </Badge>
                <Badge variant={getAppointmentConfirmationBadgeVariant(item.confirmationStatus)}>
                  {getAppointmentConfirmationLabel(item.confirmationStatus)}
                </Badge>
                <Badge variant={item.paymentStatus === "paid" ? "success" : item.paymentStatus === "pending" ? "warning" : "secondary"}>
                  {item.paymentStatus ? getPaymentStatusLabel(item.paymentStatus) : "Sem pagamento"}
                </Badge>
                {item.seriesId ? <Badge variant="outline">Serie recorrente</Badge> : null}
              </div>
            </div>
            <div className="flex flex-col items-end gap-2 text-right">
              <div>
                <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">Valor</p>
                <p className="font-semibold">{formatCurrencyBRL(item.sessionPriceCents)}</p>
              </div>
            </div>
          </div>

          <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_260px]">
            <div className="rounded-3xl bg-muted/35 p-4 text-sm leading-6 text-slate-700">
              {item.summary || "Sem registro textual para este atendimento ainda."}
            </div>
            <div className="rounded-3xl bg-muted/35 p-4">
              <p className="mb-3 text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                Anexos
              </p>
              <div className="space-y-2">
                {item.files.length ? (
                  item.files.map((file) => (
                    <a
                      key={file.id}
                      className="flex items-center gap-2 text-sm font-medium text-primary"
                      href={`/api/files/${file.id}/download`}
                    >
                      <FileText className="h-4 w-4" />
                      {file.originalName}
                      <span className="text-xs font-normal text-muted-foreground">
                        ({getRecordFileKindLabel(file.kind)})
                      </span>
                    </a>
                  ))
                ) : (
                  <p className="text-sm text-muted-foreground">Nenhum anexo vinculado.</p>
                )}
              </div>
            </div>
          </div>
        </div>
      ))}
    </>
  );
}
