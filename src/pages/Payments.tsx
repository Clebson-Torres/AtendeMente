import { useEffect, useState } from "react";
import { api, type PaymentWithAppointment } from "../lib/api";
import DataTable from "../components/ui/DataTable";
import type { Column } from "../components/ui/DataTable";
import Button from "../components/ui/Button";
import Modal from "../components/ui/Modal";
import Input from "../components/ui/Input";
import TextArea from "../components/ui/TextArea";
import Select from "../components/ui/Select";
import StatusBadge from "../components/ui/StatusBadge";
import { toast } from "../components/ui/Toast";
import { formatBRL, formatDate, formatTime } from "../lib/format";
import { useNavigate } from "react-router-dom";
import { CreditCard } from "lucide-react";

export default function Payments() {
  const navigate = useNavigate();
  const [payments, setPayments] = useState<PaymentWithAppointment[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [summary, setSummary] = useState({ paid_cents: 0, pending_cents: 0 });

  const [modalOpen, setModalOpen] = useState(false);
  const [editPayment, setEditPayment] = useState<PaymentWithAppointment | null>(null);
  const [payStatus, setPayStatus] = useState("paid");
  const [payMethod, setPayMethod] = useState("pix");
  const [payAmount, setPayAmount] = useState(0);
  const [payNotes, setPayNotes] = useState("");
  const [saving, setSaving] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const [list, sum] = await Promise.all([api.payments.list(), api.payments.summary()]);
      setPayments(list);
      setSummary(sum);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  function openEdit(p: PaymentWithAppointment) {
    setEditPayment(p);
    setPayStatus(p.status || "paid");
    setPayMethod(p.method || "pix");
    setPayAmount(p.amount_received_cents || p.session_price_cents);
    setPayNotes("");
    setModalOpen(true);
  }

  async function handleSave() {
    setSaving(true);
    try {
      await api.payments.upsert({
        appointment_id: editPayment!.appointment_id,
        status: payStatus,
        method: payMethod,
        paid_at: payStatus === "paid" ? new Date().toISOString() : null,
        amount_received_cents: payAmount,
        notes: payNotes || undefined,
      });
      toast("Pagamento salvo.");
      setModalOpen(false);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setSaving(false);
    }
  }

  const columns: Column<PaymentWithAppointment>[] = [
    { key: "patient_name", header: "Paciente", className: "font-medium" },
    { key: "starts_at", header: "Data", render: (p) => formatDate(p.starts_at) },
    { key: "starts_at", header: "Horário", render: (p) => formatTime(p.starts_at) },
    { key: "session_price_cents", header: "Valor", render: (p) => formatBRL(p.session_price_cents) },
    {
      key: "amount_received_cents",
      header: "Recebido",
      render: (p) => p.amount_received_cents ? formatBRL(p.amount_received_cents) : "-",
    },
    {
      key: "status",
      header: "Status",
      render: (p) => <StatusBadge status={p.status || "unpaid"} />,
    },
    {
      key: "actions",
      header: "",
      render: (p) => (
        <div className="flex gap-2 justify-end">
          <Button variant="ghost" size="sm" onClick={() => openEdit(p)}>{p.status ? "Editar" : "Pagar"}</Button>
          <Button variant="ghost" size="sm" onClick={() => navigate(`/appointments/${p.appointment_id}`)}>Detalhes</Button>
        </div>
      ),
    },
  ];

  return (
    <div className="p-4 sm:p-6 space-y-5">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <CreditCard className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-display font-semibold text-slate-900">Financeiro</h1>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">Total de Atendimentos</p>
          <p className="text-3xl font-bold text-primary mt-1">{payments.length}</p>
        </div>
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">A receber</p>
          <p className="text-3xl font-bold text-yellow-600 mt-1">{formatBRL(summary.pending_cents)}</p>
        </div>
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">Recebido</p>
          <p className="text-3xl font-bold text-success mt-1">{formatBRL(summary.paid_cents)}</p>
        </div>
      </div>

      {error && <p className="text-destructive text-sm">{error}</p>}

      {loading ? (
        <div className="text-center py-12 text-muted-foreground">Carregando...</div>
      ) : (
        <div className="app-surface overflow-hidden">
          <DataTable
            columns={columns}
            data={payments}
            keyExtractor={(p) => p.appointment_id}
            emptyMessage="Nenhum atendimento financeiro encontrado."
          />
        </div>
      )}

      <Modal open={modalOpen} onClose={() => setModalOpen(false)} title="Registrar Pagamento">
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground font-medium">{editPayment?.patient_name} - {formatDate(editPayment?.starts_at || "")}</p>
          <div className="grid grid-cols-2 gap-4">
            <Select label="Status" value={payStatus} onChange={(e) => setPayStatus(e.target.value)} options={[
              { value: "paid", label: "Pago" },
              { value: "unpaid", label: "Pendente" },
              { value: "partial", label: "Parcial" },
            ]} />
            <Select label="Método" value={payMethod} onChange={(e) => setPayMethod(e.target.value)} options={[
              { value: "pix", label: "PIX" },
              { value: "credit_card", label: "Cartão de Crédito" },
              { value: "debit_card", label: "Cartão de Débito" },
              { value: "cash", label: "Dinheiro" },
              { value: "transfer", label: "Transferência" },
              { value: "other", label: "Outro" },
            ]} />
          </div>
          <Input label="Valor (R$)" type="number" step="0.01" value={payAmount / 100} onChange={(e) => setPayAmount(Math.round(parseFloat(e.target.value || "0") * 100))} />
          <TextArea label="Observações" rows={2} value={payNotes} onChange={(e) => setPayNotes(e.target.value)} />
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="outline" onClick={() => setModalOpen(false)}>Cancelar</Button>
            <Button onClick={handleSave} disabled={saving}>{saving ? "Salvando..." : "Salvar"}</Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
