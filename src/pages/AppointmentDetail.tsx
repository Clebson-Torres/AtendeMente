import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { api, type AppointmentDetail as AppointmentDetailType } from "../lib/api";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import TextArea from "../components/ui/TextArea";
import StatusBadge from "../components/ui/StatusBadge";
import ConfirmDialog from "../components/ui/ConfirmDialog";
import FieldError from "../components/ui/FieldError";
import { toast } from "../components/ui/Toast";
import { formatBRL, formatDateTime } from "../lib/format";
import { paymentSchema, type PaymentInput } from "../lib/schemas";
import { ArrowLeft, Lock, Calendar, Upload, File } from "lucide-react";
import RescheduleDialog from "../components/RescheduleDialog";
import FileUploadButton from "../components/FileUploadButton";
import FileList from "../components/FileList";

export default function AppointmentDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [appt, setAppt] = useState<AppointmentDetailType | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const [payModal, setPayModal] = useState(false);
  const [paying, setPaying] = useState(false);

  const { register: regPay, handleSubmit: handlePaySubmit, reset: resetPay, formState: { errors: payErrors } } = useForm<PaymentInput>({
    resolver: zodResolver(paymentSchema),
    defaultValues: { status: "paid", method: "pix", amount_received_cents: 0 },
  });

  const [recordContent, setRecordContent] = useState("");
  const [recordLoading, setRecordLoading] = useState(false);
  const [recordSaving, setRecordSaving] = useState(false);

  const [cancelOpen, setCancelOpen] = useState(false);
  const [cancelReason, setCancelReason] = useState("");
  const [cancelling, setCancelling] = useState(false);

  const [completing, setCompleting] = useState(false);
  const [noShowing, setNoShowing] = useState(false);
  const [confirming, setConfirming] = useState(false);

  const [rescheduleOpen, setRescheduleOpen] = useState(false);

  async function load() {
    if (!id) return;
    setLoading(true);
    try {
      const data = await api.appointments.get(id);
      setAppt(data);
      resetPay({ status: "paid", method: "pix", amount_received_cents: data.session_price_cents / 100 });
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  async function loadRecord() {
    if (!id) return;
    setRecordLoading(true);
    try {
      const content = await api.records.get(id);
      setRecordContent(content);
    } catch { setRecordContent(""); }
    finally { setRecordLoading(false); }
  }

  useEffect(() => { load(); }, [id]);

  async function onPay(data: PaymentInput) {
    if (!id) return;
    setPaying(true);
    try {
      await api.payments.upsert({
        appointment_id: id,
        status: data.status,
        method: data.method,
        paid_at: data.status === "paid" ? new Date().toISOString() : null,
        amount_received_cents: Math.round(data.amount_received_cents * 100),
        notes: data.notes || undefined,
      });
      toast("Pagamento registrado.");
      setPayModal(false);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setPaying(false);
    }
  }

  async function handleSaveRecord() {
    if (!id || !appt) return;
    setRecordSaving(true);
    try {
      await api.records.save({ appointment_id: id, patient_id: appt.patient_id, content: recordContent });
      toast("Prontuário salvo com segurança.");
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setRecordSaving(false);
    }
  }

  async function handleCancel() {
    if (!id) return;
    setCancelling(true);
    try {
      await api.appointments.cancel(id, cancelReason || undefined);
      toast("Atendimento cancelado.");
      setCancelOpen(false);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setCancelling(false);
    }
  }

  async function handleComplete() {
    if (!id) return;
    setCompleting(true);
    try {
      await api.appointments.update(id, { status: "completed" });
      toast("Atendimento concluído.");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setCompleting(false);
    }
  }

  async function handleNoShow() {
    if (!id) return;
    setNoShowing(true);
    try {
      await api.appointments.update(id, { status: "no_show" });
      toast("Presença não confirmada.");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setNoShowing(false);
    }
  }

  async function handleToggleConfirm() {
    if (!id || !appt) return;
    setConfirming(true);
    try {
      const newStatus = appt.confirmation_status === "confirmed" ? "unconfirmed" : "confirmed";
      await api.appointments.update(id, { confirmation_status: newStatus });
      toast(newStatus === "confirmed" ? "Atendimento confirmado." : "Confirmação removida.");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setConfirming(false);
    }
  }

  if (loading) return <div className="p-6 text-muted-foreground">Carregando...</div>;
  if (error) return <div className="p-6 text-destructive">{error}</div>;
  if (!appt) return <div className="p-6 text-muted-foreground">Atendimento não encontrado.</div>;

  const isCancelled = appt.status === "cancelled";
  const isScheduled = appt.status === "scheduled";

  return (
    <div className="p-4 sm:p-6 space-y-6 max-w-4xl">
      <button onClick={() => navigate("/appointments")} className="flex items-center gap-1 text-sm text-primary hover:text-primary/80 transition-colors">
        <ArrowLeft className="h-4 w-4" /> Voltar para Agenda
      </button>

      <div className="flex items-start justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-2xl font-display font-semibold text-slate-900">{appt.patient_name}</h1>
          <p className="text-sm text-muted-foreground">{formatDateTime(appt.starts_at)} - {appt.ends_at.slice(11, 16)}</p>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={appt.status} />
          <StatusBadge status={appt.confirmation_status} />
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="app-surface p-5">
          <h2 className="font-semibold text-slate-900 mb-3">Pagamento</h2>
          <div className="text-sm space-y-2">
            <p><span className="text-muted-foreground">Valor da sessão:</span> <span className="font-medium">{formatBRL(appt.session_price_cents)}</span></p>
            {appt.payment_status ? (
              <>
                <p><span className="text-muted-foreground">Status:</span> <StatusBadge status={appt.payment_status} /></p>
                <p><span className="text-muted-foreground">Método:</span> {appt.payment_method || "-"}</p>
                <p><span className="text-muted-foreground">Recebido:</span> {appt.amount_received_cents ? formatBRL(appt.amount_received_cents) : "-"}</p>
                <p><span className="text-muted-foreground">Data:</span> {formatDateTime(appt.paid_at)}</p>
              </>
            ) : <p className="text-muted-foreground">Nenhum pagamento registrado.</p>}
          </div>
          {!isCancelled && (
            <Button onClick={() => setPayModal(true)} className="mt-3">{appt.payment_status ? "Editar Pagamento" : "Registrar Pagamento"}</Button>
          )}
        </div>
        {!isCancelled && (
          <div className="app-surface p-5 space-y-3">
            <h2 className="font-semibold text-slate-900 mb-3">Ações</h2>
            {isScheduled && (
              <>
                <Button onClick={handleComplete} disabled={completing} className="w-full">
                  {completing ? "Concluindo..." : "Concluir Atendimento"}
                </Button>
                <Button onClick={handleNoShow} disabled={noShowing} variant="outline" className="w-full">
                  {noShowing ? "Registrando..." : "Não Compareceu"}
                </Button>
                <Button onClick={handleToggleConfirm} disabled={confirming} variant="outline" className="w-full">
                  {confirming ? "Alterando..." : appt.confirmation_status === "confirmed" ? "Remover Confirmação" : "Confirmar Agendamento"}
                </Button>
              </>
            )}
            <Button onClick={() => setRescheduleOpen(true)} className="w-full">
              <Calendar className="h-4 w-4 mr-2" /> Reagendar
            </Button>
            <Button variant="destructive" onClick={() => setCancelOpen(true)} className="w-full">Cancelar Atendimento</Button>
          </div>
        )}
      </div>

      <div className="app-surface p-5">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-semibold text-slate-900">Arquivos</h2>
          <FileUploadButton appointmentId={appt.id} patientId={appt.patient_id} onUploadComplete={load} />
        </div>
        <FileList appointmentId={appt.id} />
      </div>

      <div className="app-surface p-5">
        <div className="flex items-center gap-2 mb-3">
          <h2 className="font-semibold text-slate-900">Prontuário da Sessão</h2>
          <span className="flex items-center gap-1 text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded-full"><Lock className="h-3 w-3" /> Criptografado</span>
        </div>
        {recordLoading ? (
          <p className="text-muted-foreground text-sm">Carregando...</p>
        ) : (
          <>
            <TextArea rows={12} placeholder="Registre aqui as anotações da sessão. O conteúdo será criptografado automaticamente." value={recordContent} onChange={(e) => setRecordContent(e.target.value)} className="font-mono text-sm" />
            <div className="flex justify-end mt-3">
              <Button onClick={handleSaveRecord} disabled={recordSaving}>{recordSaving ? "Salvando..." : "Salvar Prontuário"}</Button>
            </div>
          </>
        )}
      </div>

      <ConfirmDialog open={payModal} onClose={() => setPayModal(false)} onConfirm={handlePaySubmit(onPay)} title="Registrar Pagamento" message="" confirmLabel="Salvar" loading={paying}>
        <form onSubmit={handlePaySubmit(onPay)} className="space-y-4" id="pay-form">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-800 mb-1">Status</label>
              <select {...regPay("status")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1">
                <option value="paid">Pago</option>
                <option value="pending">Pendente</option>
                <option value="cancelled">Cancelado</option>
              </select>
              <FieldError message={payErrors.status?.message} />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-800 mb-1">Método</label>
              <select {...regPay("method")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1">
                <option value="pix">PIX</option>
                <option value="card">Cartão</option>
                <option value="cash">Dinheiro</option>
                <option value="bank_transfer">Transferência</option>
                <option value="other">Outro</option>
              </select>
              <FieldError message={payErrors.method?.message} />
            </div>
          </div>
          <div>
            <Input label="Valor Recebido (R$)" type="number" step="0.01" {...regPay("amount_received_cents", { valueAsNumber: true })} />
            <FieldError message={payErrors.amount_received_cents?.message} />
          </div>
          <TextArea label="Observações" rows={2} {...regPay("notes")} />
        </form>
      </ConfirmDialog>

      <RescheduleDialog
        open={rescheduleOpen}
        onClose={() => setRescheduleOpen(false)}
        appointmentId={appt.id}
        currentStart={appt.starts_at}
        currentEnd={appt.ends_at}
        onRescheduled={load}
      />

      <ConfirmDialog open={cancelOpen} onClose={() => setCancelOpen(false)} onConfirm={handleCancel} title="Cancelar Atendimento" message="Tem certeza que deseja cancelar este atendimento?" confirmLabel="Cancelar Atendimento" loading={cancelling}>
        <div className="mt-4"><Input label="Motivo do cancelamento (opcional)" value={cancelReason} onChange={(e) => setCancelReason(e.target.value)} /></div>
      </ConfirmDialog>
    </div>
  );
}
