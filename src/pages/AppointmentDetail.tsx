import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { api, type AppointmentDetail as AppointmentDetailType, type SaveRecordInput } from "../lib/api";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import TextArea from "../components/ui/TextArea";
import StatusBadge from "../components/ui/StatusBadge";
import ConfirmDialog from "../components/ui/ConfirmDialog";
import { toast } from "../components/ui/Toast";
import { formatBRL, formatDateTime } from "../lib/format";
import { ArrowLeft, Lock } from "lucide-react";

export default function AppointmentDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [appt, setAppt] = useState<AppointmentDetailType | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const [payModal, setPayModal] = useState(false);
  const [payStatus, setPayStatus] = useState("paid");
  const [payMethod, setPayMethod] = useState("pix");
  const [payAmount, setPayAmount] = useState(0);
  const [payNotes, setPayNotes] = useState("");
  const [paying, setPaying] = useState(false);

  const [recordContent, setRecordContent] = useState("");
  const [recordLoading, setRecordLoading] = useState(false);
  const [recordSaving, setRecordSaving] = useState(false);

  const [cancelOpen, setCancelOpen] = useState(false);
  const [cancelReason, setCancelReason] = useState("");
  const [cancelling, setCancelling] = useState(false);

  async function load() {
    if (!id) return;
    setLoading(true);
    try {
      const data = await api.appointments.get(id);
      setAppt(data);
      setPayAmount(data.session_price_cents);
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
    } catch {
      setRecordContent("");
    } finally {
      setRecordLoading(false);
    }
  }

  useEffect(() => { load(); }, [id]);

  async function handlePay() {
    if (!id) return;
    setPaying(true);
    try {
      await api.payments.upsert({
        appointment_id: id,
        status: payStatus,
        method: payMethod,
        paid_at: payStatus === "paid" ? new Date().toISOString() : null,
        amount_received_cents: payAmount,
        notes: payNotes || undefined,
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
      await api.records.save({
        appointment_id: id,
        patient_id: appt.patient_id,
        content: recordContent,
      });
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

  if (loading) return <div className="p-6 text-muted-foreground">Carregando...</div>;
  if (error) return <div className="p-6 text-destructive">{error}</div>;
  if (!appt) return <div className="p-6 text-muted-foreground">Atendimento não encontrado.</div>;

  const isCancelled = appt.status === "cancelled";

  return (
    <div className="p-4 sm:p-6 space-y-6 max-w-4xl">
      <button
        onClick={() => navigate("/appointments")}
        className="flex items-center gap-1 text-sm text-primary hover:text-primary/80 transition-colors"
      >
        <ArrowLeft className="h-4 w-4" />
        Voltar para Agenda
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
            ) : (
              <p className="text-muted-foreground">Nenhum pagamento registrado.</p>
            )}
          </div>
          {!isCancelled && (
            <Button onClick={() => setPayModal(true)} className="mt-3">{appt.payment_status ? "Editar Pagamento" : "Registrar Pagamento"}</Button>
          )}
        </div>

        {!isCancelled && (
          <div className="app-surface p-5">
            <h2 className="font-semibold text-slate-900 mb-3">Ações</h2>
            <Button variant="destructive" onClick={() => setCancelOpen(true)}>Cancelar Atendimento</Button>
          </div>
        )}
      </div>

      <div className="app-surface p-5">
        <div className="flex items-center gap-2 mb-3">
          <h2 className="font-semibold text-slate-900">Prontuário da Sessão</h2>
          <span className="flex items-center gap-1 text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded-full">
            <Lock className="h-3 w-3" />
            Criptografado
          </span>
        </div>
        {recordLoading ? (
          <p className="text-muted-foreground text-sm">Carregando...</p>
        ) : (
          <>
            <TextArea
              rows={12}
              placeholder="Registre aqui as anotações da sessão. O conteúdo será criptografado automaticamente."
              value={recordContent}
              onChange={(e) => setRecordContent(e.target.value)}
              className="font-mono text-sm"
            />
            <div className="flex justify-end mt-3">
              <Button onClick={handleSaveRecord} disabled={recordSaving}>{recordSaving ? "Salvando..." : "Salvar Prontuário"}</Button>
            </div>
          </>
        )}
      </div>

      <ConfirmDialog
        open={payModal}
        onClose={() => setPayModal(false)}
        onConfirm={handlePay}
        title="Registrar Pagamento"
        message=""
        confirmLabel="Salvar"
        loading={paying}
      >
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-800 mb-1">Status</label>
              <select value={payStatus} onChange={(e) => setPayStatus(e.target.value)} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1">
                <option value="paid">Pago</option>
                <option value="unpaid">Pendente</option>
                <option value="partial">Parcial</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-800 mb-1">Método</label>
              <select value={payMethod} onChange={(e) => setPayMethod(e.target.value)} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1">
                <option value="pix">PIX</option>
                <option value="credit_card">Cartão de Crédito</option>
                <option value="debit_card">Cartão de Débito</option>
                <option value="cash">Dinheiro</option>
                <option value="transfer">Transferência</option>
                <option value="other">Outro</option>
              </select>
            </div>
          </div>
          <Input label="Valor Recebido (R$)" type="number" step="0.01" value={payAmount / 100} onChange={(e) => setPayAmount(Math.round(parseFloat(e.target.value || "0") * 100))} />
          <TextArea label="Observações" rows={2} value={payNotes} onChange={(e) => setPayNotes(e.target.value)} />
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={cancelOpen}
        onClose={() => setCancelOpen(false)}
        onConfirm={handleCancel}
        title="Cancelar Atendimento"
        message="Tem certeza que deseja cancelar este atendimento?"
        confirmLabel="Cancelar Atendimento"
        loading={cancelling}
      >
        <div className="mt-4">
          <Input label="Motivo do cancelamento (opcional)" value={cancelReason} onChange={(e) => setCancelReason(e.target.value)} />
        </div>
      </ConfirmDialog>
    </div>
  );
}
