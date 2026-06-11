import { useEffect, useState } from "react";
import { api, type PatientListItem, type Patient, type CreatePatientInput } from "../lib/api";
import DataTable from "../components/ui/DataTable";
import type { Column } from "../components/ui/DataTable";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import TextArea from "../components/ui/TextArea";
import Modal from "../components/ui/Modal";
import StatusBadge from "../components/ui/StatusBadge";
import ConfirmDialog from "../components/ui/ConfirmDialog";
import { toast } from "../components/ui/Toast";

export default function Patients() {
  const [patients, setPatients] = useState<PatientListItem[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const [modalOpen, setModalOpen] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [form, setForm] = useState<CreatePatientInput>({ full_name: "" });
  const [saving, setSaving] = useState(false);

  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<() => void>(() => {});
  const [confirmTitle, setConfirmTitle] = useState("");

  async function load(searchTerm = "") {
    setLoading(true);
    setError("");
    try {
      const data = await api.patients.list(searchTerm || undefined);
      setPatients(data);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  function openCreate() {
    setEditId(null);
    setForm({ full_name: "" });
    setModalOpen(true);
  }

  async function openEdit(id: string) {
    try {
      const p = await api.patients.get(id);
      setEditId(id);
      setForm({
        full_name: p.full_name,
        chart_number: p.chart_number,
        phone: p.phone,
        email: p.email,
        birth_date: p.birth_date,
        health_history: p.health_history,
        medications_in_use: p.medications_in_use,
        emergency_phone: p.emergency_phone,
        admin_notes: p.admin_notes,
      });
      setModalOpen(true);
    } catch (e: any) {
      toast(e.message, "error");
    }
  }

  async function handleSave() {
    if (!form.full_name.trim()) { toast("Nome é obrigatório", "error"); return; }
    setSaving(true);
    try {
      if (editId) {
        await api.patients.update(editId, form);
        toast("Paciente atualizado.");
      } else {
        await api.patients.create(form);
        toast("Paciente cadastrado.");
      }
      setModalOpen(false);
      load(search);
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setSaving(false);
    }
  }

  function confirmDeactivate(p: PatientListItem) {
    setConfirmTitle(`Desativar ${p.full_name}?`);
    setConfirmAction(() => async () => {
      try {
        await api.patients.deactivate(p.id);
        toast("Paciente desativado.");
        load(search);
      } catch (e: any) { toast(e.message, "error"); }
    });
    setConfirmOpen(true);
  }

  function confirmActivate(p: PatientListItem) {
    setConfirmTitle(`Reativar ${p.full_name}?`);
    setConfirmAction(() => async () => {
      try {
        await api.patients.activate(p.id);
        toast("Paciente reativado.");
        load(search);
      } catch (e: any) { toast(e.message, "error"); }
    });
    setConfirmOpen(true);
  }

  const columns: Column<PatientListItem>[] = [
    { key: "full_name", header: "Nome", className: "font-medium" },
    { key: "chart_number", header: "Prontuário" },
    { key: "phone", header: "Telefone" },
    {
      key: "birth_date",
      header: "Nascimento",
      render: (p) => p.birth_date ? new Date(p.birth_date).toLocaleDateString("pt-BR") : "-",
    },
    {
      key: "status",
      header: "Status",
      render: (p) => <StatusBadge status={p.status} />,
    },
    {
      key: "actions",
      header: "",
      render: (p) => (
        <div className="flex gap-2 justify-end">
          <Button variant="ghost" onClick={() => openEdit(p.id)}>Editar</Button>
          {p.status === "active" ? (
            <Button variant="ghost" onClick={() => confirmDeactivate(p)}>Desativar</Button>
          ) : (
            <Button variant="ghost" onClick={() => confirmActivate(p)}>Reativar</Button>
          )}
        </div>
      ),
    },
  ];

  return (
    <div className="p-6 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-800">Pacientes</h1>
        <Button onClick={openCreate}>+ Novo Paciente</Button>
      </div>

      <Input
        placeholder="Buscar por nome, telefone ou email..."
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          load(e.target.value);
        }}
      />

      {error && <p className="text-red-500 text-sm">{error}</p>}

      {loading ? (
        <div className="text-center py-12 text-gray-400">Carregando...</div>
      ) : (
        <div className="bg-white rounded-xl shadow-sm border border-gray-100">
          <DataTable
            columns={columns}
            data={patients}
            keyExtractor={(p) => p.id}
            emptyMessage="Nenhum paciente cadastrado. Clique em '+ Novo Paciente' para começar."
          />
        </div>
      )}

      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={editId ? "Editar Paciente" : "Novo Paciente"}
      >
        <div className="space-y-4">
          <Input label="Nome completo *" value={form.full_name} onChange={(e) => setForm({ ...form, full_name: e.target.value })} />
          <div className="grid grid-cols-2 gap-4">
            <Input label="Nº Prontuário" value={form.chart_number || ""} onChange={(e) => setForm({ ...form, chart_number: e.target.value || null })} />
            <Input label="Telefone" value={form.phone || ""} onChange={(e) => setForm({ ...form, phone: e.target.value || null })} />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <Input label="Email" type="email" value={form.email || ""} onChange={(e) => setForm({ ...form, email: e.target.value || null })} />
            <Input label="Data de Nascimento" type="date" value={form.birth_date || ""} onChange={(e) => setForm({ ...form, birth_date: e.target.value || null })} />
          </div>
          <Input label="Telefone de Emergência" value={form.emergency_phone || ""} onChange={(e) => setForm({ ...form, emergency_phone: e.target.value || null })} />
          <TextArea label="Histórico de Saúde" rows={3} value={form.health_history || ""} onChange={(e) => setForm({ ...form, health_history: e.target.value || null })} />
          <TextArea label="Medicações em Uso" rows={3} value={form.medications_in_use || ""} onChange={(e) => setForm({ ...form, medications_in_use: e.target.value || null })} />
          <TextArea label="Observações Administrativas" rows={3} value={form.admin_notes || ""} onChange={(e) => setForm({ ...form, admin_notes: e.target.value || null })} />
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>Cancelar</Button>
            <Button onClick={handleSave} disabled={saving}>{saving ? "Salvando..." : "Salvar"}</Button>
          </div>
        </div>
      </Modal>

      <ConfirmDialog
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={confirmAction}
        title={confirmTitle}
        message="Essa ação pode ser revertida depois."
        confirmLabel="Confirmar"
      />
    </div>
  );
}
