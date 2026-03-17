"use client";

import { useMemo, useRef, useState } from "react";
import { AppointmentsCalendar } from "@/components/calendar/appointments-calendar";
import { AppointmentForm } from "@/components/forms/appointment-form";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { describeDateTimeRangeBR, formatDateBR, toDateTimeLocalValue } from "@/lib/utils";

type AgendaWorkspaceProps = {
  patientOptions: Array<{ id: string; fullName: string }>;
};

function plusOneHour(dateString: string) {
  const date = new Date(dateString);
  date.setHours(date.getHours() + 1);
  return toDateTimeLocalValue(date);
}

export function AgendaWorkspace({ patientOptions }: AgendaWorkspaceProps) {
  const [selectedSlot, setSelectedSlot] = useState<{ startsAt: string; endsAt: string } | null>(null);
  const refetchCalendarRef = useRef<(() => void) | null>(null);

  const defaultValues = useMemo(
    () => ({
      startsAt: selectedSlot?.startsAt ?? "",
      endsAt: selectedSlot?.endsAt ?? "",
      quickNotes: "",
      cancelReason: "",
      sessionPriceCents: "180,00",
      status: "scheduled" as const,
    }),
    [selectedSlot],
  );

  return (
    <div className="grid gap-6 xl:grid-cols-[1.35fr_420px]">
      <Card>
        <CardHeader>
          <CardTitle>Calendario de atendimentos</CardTitle>
          <CardDescription>
            A agenda abre no dia atual. Clique em um horario para preencher um atendimento unico ou gerar uma serie recorrente.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <AppointmentsCalendar
            onRefetch={(fn) => { refetchCalendarRef.current = fn; }}
            onSlotSelect={(slot) =>
              setSelectedSlot({
                startsAt: toDateTimeLocalValue(slot.start),
                endsAt: toDateTimeLocalValue(slot.end || plusOneHour(slot.start)),
              })
            }
          />
        </CardContent>
      </Card>

        <Card>
        <CardHeader>
          <CardTitle>
            {selectedSlot ? `Novo atendimento em ${formatDateBR(selectedSlot.startsAt)}` : "Novo atendimento"}
          </CardTitle>
          <CardDescription>
            {selectedSlot
              ? `Horario selecionado: ${describeDateTimeRangeBR(selectedSlot.startsAt, selectedSlot.endsAt)}.`
              : "Escolha um paciente e informe o horario. O padrao do MVP e de 1 hora por sessao, com opcao de recorrencia semanal ou quinzenal."}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <AppointmentForm
            afterSuccess={() => { setSelectedSlot(null); refetchCalendarRef.current?.(); }}
            defaultValues={defaultValues}
            patientOptions={patientOptions}
          />
        </CardContent>
      </Card>
    </div>
  );
}
