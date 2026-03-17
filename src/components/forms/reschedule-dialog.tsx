"use client";

import { useEffect, useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { updateAppointmentAction } from "@/features/appointments/actions";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  addOneHourToTimeInput,
  combineDateAndTimeInput,
  formatDateInputBR,
  maskDateInputBR,
  maskTimeInput,
  splitDateTimeInput,
} from "@/lib/utils";
import { FieldError } from "@/components/forms/field-error";

type RescheduleDialogProps = {
  appointmentId: string | null;
  patientId: string;
  currentStartsAt: string;
  currentEndsAt: string;
  onOpenChange: (open: boolean) => void;
  afterSuccess?: () => void;
};

export function RescheduleDialog({
  appointmentId,
  patientId,
  currentStartsAt,
  currentEndsAt,
  onOpenChange,
  afterSuccess,
}: RescheduleDialogProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  const [dateInput, setDateInput] = useState(() => formatDateInputBR(currentStartsAt));
  const [startTimeInput, setStartTimeInput] = useState(() => splitDateTimeInput(currentStartsAt).time);
  const [endTimeInput, setEndTimeInput] = useState(() => splitDateTimeInput(currentEndsAt).time);
  const [reason, setReason] = useState("");
  const [errors, setErrors] = useState<{ startsAt?: string; endsAt?: string }>({});

  useEffect(() => {
    if (appointmentId) {
      setDateInput(formatDateInputBR(currentStartsAt));
      setStartTimeInput(splitDateTimeInput(currentStartsAt).time);
      setEndTimeInput(splitDateTimeInput(currentEndsAt).time);
      setReason("");
      setErrors({});
    }
  }, [appointmentId, currentStartsAt, currentEndsAt]);

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (!appointmentId) return;

    const startsAt = combineDateAndTimeInput(dateInput, startTimeInput);
    const endsAt = combineDateAndTimeInput(dateInput, endTimeInput);

    if (!startsAt || !endsAt) {
      setErrors({ startsAt: "Informe datas válidas." });
      return;
    }

    startTransition(async () => {
      const result = await updateAppointmentAction(appointmentId, {
        patientId,
        startsAt,
        endsAt,
        status: "scheduled",
        confirmationStatus: "unconfirmed",
        sessionPriceCents: "0", 
        quickNotes: reason,
      });

      if (!result.success) {
        toast.error(result.message);
        return;
      }

      toast.success("Atendimento reagendado com sucesso!");
      onOpenChange(false);

      if (afterSuccess) {
        afterSuccess();
      }
      
      router.refresh();
    });
  };

  return (
    <Dialog open={!!appointmentId} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Reagendar Atendimento</DialogTitle>
          <DialogDescription>
            Escolha o novo dia e horário para a sessão.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-5">
          <div className="space-y-2">
            <Label htmlFor="reschedule-date">Nova Data</Label>
            <Input
              id="reschedule-date"
              inputMode="numeric"
              placeholder="dd/mm/aaaa"
              value={dateInput}
              onChange={(e) => {
                const el = e.target;
                const pos = el.selectionStart;
                const val = el.value;
                setDateInput(maskDateInputBR(val));
                
                // Avoid cursor jumping to the end
                window.requestAnimationFrame(() => {
                  if (el && pos !== null) {
                    // Small heuristic to jump over automatically inserted slashes
                    const newPos = val.length < maskDateInputBR(val).length && (pos === 2 || pos === 5) ? pos + 1 : pos;
                    el.setSelectionRange(newPos, newPos);
                  }
                });
              }}
            />
            <FieldError message={errors.startsAt} />
          </div>

          <div className="grid gap-5 md:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="reschedule-start">Horário de início</Label>
              <Input
                id="reschedule-start"
                inputMode="numeric"
                placeholder="hh:mm"
                value={startTimeInput}
                onChange={(e) => {
                  const nextStart = maskTimeInput(e.target.value);
                  setStartTimeInput(nextStart);
                  setEndTimeInput(addOneHourToTimeInput(nextStart));
                }}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="reschedule-end">Horário final</Label>
              <Input
                id="reschedule-end"
                inputMode="numeric"
                placeholder="hh:mm"
                value={endTimeInput}
                onChange={(e) => setEndTimeInput(maskTimeInput(e.target.value))}
              />
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="reschedule-reason">Motivo do Reagendamento (opcional)</Label>
            <Textarea
              id="reschedule-reason"
              placeholder="Ex: Paciente pediu para adiar por motivo de saúde."
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            />
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <Button
              type="button"
              variant="outline"
              disabled={isPending}
              onClick={() => onOpenChange(false)}
            >
              Cancelar
            </Button>
            <Button type="submit" disabled={isPending}>
              {isPending ? "Salvando..." : "Confirmar Reagendamento"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
