import { useState } from "react";
import Modal from "./ui/Modal";
import Button from "./ui/Button";
import Input from "./ui/Input";
import { toast } from "./ui/Toast";
import { api } from "../lib/api";

interface Props {
  open: boolean;
  onClose: () => void;
  appointmentId: string;
  currentStart: string;
  currentEnd: string;
  onRescheduled: () => void;
}

export default function RescheduleDialog({ open, onClose, appointmentId, currentStart, currentEnd, onRescheduled }: Props) {
  const [startsAt, setStartsAt] = useState(currentStart.slice(0, 16));
  const [endsAt, setEndsAt] = useState(currentEnd.slice(0, 16));
  const [saving, setSaving] = useState(false);

  async function handleSave() {
    if (!startsAt || !endsAt) { toast("Preencha início e fim.", "error"); return; }
    if (startsAt >= endsAt) { toast("O fim deve ser após o início.", "error"); return; }
    setSaving(true);
    try {
      await api.appointments.update(appointmentId, { patient_id: "", starts_at: startsAt, ends_at: endsAt });
      toast("Atendimento reagendado.");
      onRescheduled();
      onClose();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="Reagendar Atendimento" size="sm">
      <div className="space-y-4">
        <Input label="Início" type="datetime-local" value={startsAt} onChange={(e) => setStartsAt(e.target.value)} />
        <Input label="Fim" type="datetime-local" value={endsAt} onChange={(e) => setEndsAt(e.target.value)} />
        <div className="flex justify-end gap-3 pt-2">
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={handleSave} disabled={saving}>{saving ? "Salvando..." : "Reagendar"}</Button>
        </div>
      </div>
    </Modal>
  );
}
