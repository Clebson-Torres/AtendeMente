import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { api, type PatientListItem, type CreatePatientInput } from "../lib/api";
import DataTable from "../components/ui/DataTable";
import type { Column } from "../components/ui/DataTable";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import TextArea from "../components/ui/TextArea";
import Modal from "../components/ui/Modal";
import StatusBadge from "../components/ui/StatusBadge";
import ConfirmDialog from "../components/ui/ConfirmDialog";
import FieldError from "../components/ui/FieldError";
import { toast } from "../components/ui/Toast";
import { patientSchema, type PatientInput } from "../lib/schemas";
import { formatDate } from "../lib/format";
import { Upload, UsersRound, Plus, Search, FileSpreadsheet } from "lucide-react";
import ImportPatientsModal from "../components/ImportPatientsModal";

export default function Patients() {
  const navigate = useNavigate();
  const [patients, setPatients] = useState<PatientListItem[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [perPage] = useState(50);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const [modalOpen, setModalOpen] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);

  const { register, handleSubmit, reset, formState: { errors } } = useForm<PatientInput>({
    resolver: zodResolver(patientSchema),
    defaultValues: { full_name: "" },
  });

  const [saving, setSaving] = useState(false);

  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<() => void>(() => {});
  const [confirmTitle, setConfirmTitle] = useState("");
  const [importOpen, setImportOpen] = useState(false);

  async function load(searchTerm = "", p = 1) {
    setLoading(true);
    setError("");
    try {
      const result = await api.patients.list(searchTerm || undefined, p, perPage);
      setPatients(result.items);
      setTotal(result.total);
      setPage(result.page);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  function openCreate() {
    setEditId(null);
    reset({ full_name: "" });
    setModalOpen(true);
  }

  async function openEdit(id: string) {
    try {
      const p = await api.patients.get(id);
      setEditId(id);
      reset({
        full_name: p.full_name,
        chart_number: p.chart_number ?? undefined,
        phone: p.phone ?? undefined,
        email: p.email ?? undefined,
        birth_date: p.birth_date ?? undefined,
        health_history: p.health_history ?? undefined,
        medications_in_use: p.medications_in_use ?? undefined,
        emergency_phone: p.emergency_phone ?? undefined,
        admin_notes: p.admin_notes ?? undefined,
      });
      setModalOpen(true);
    } catch (e: any) {
      toast(e.message, "error");
    }
  }

  async function onSave(data: PatientInput) {
    setSaving(true);
    try {
      if (editId) {
        await api.patients.update(editId, data as CreatePatientInput);
        toast("Paciente atualizado.");
      } else {
        await api.patients.create(data as CreatePatientInput);
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
      try { await api.patients.deactivate(p.id); toast("Paciente desativado."); load(search); }
      catch (e: any) { toast(e.message, "error"); }
    });
    setConfirmOpen(true);
  }

  function confirmActivate(p: PatientListItem) {
    setConfirmTitle(`Reativar ${p.full_name}?`);
    setConfirmAction(() => async () => {
      try { await api.patients.activate(p.id); toast("Paciente reativado."); load(search); }
      catch (e: any) { toast(e.message, "error"); }
    });
    setConfirmOpen(true);
  }

  const columns: Column<PatientListItem>[] = [
    { key: "full_name", header: "Nome", className: "font-medium" },
    { key: "chart_number", header: "Prontuário" },
    { key: "phone", header: "Telefone" },
    { key: "birth_date", header: "Nascimento", render: (p) => formatDate(p.birth_date) },
    { key: "status", header: "Status", render: (p) => <StatusBadge status={p.status} /> },
    {
      key: "actions", header: "",
      render: (p) => (
        <div className="flex gap-2 justify-end">
          <Button variant="ghost" size="sm" onClick={() => navigate(`/patients/${p.id}`)}>Detalhes</Button>
          <Button variant="ghost" size="sm" onClick={() => openEdit(p.id)}>Editar</Button>
          {p.status === "active"
            ? <Button variant="ghost" size="sm" onClick={() => confirmDeactivate(p)}>Desativar</Button>
            : <Button variant="ghost" size="sm" onClick={() => confirmActivate(p)}>Reativar</Button>}
        </div>
      ),
    },
  ];

  return (
    <div className="p-4 sm:p-6 space-y-5">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <UsersRound className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-display font-semibold text-slate-900">Pacientes</h1>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => setImportOpen(true)}>
            <FileSpreadsheet className="h-4 w-4 mr-2" />Importar CSV
          </Button>
          <Button onClick={openCreate}><Plus className="h-4 w-4 mr-2" />Novo Paciente</Button>
        </div>
      </div>

      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input placeholder="Buscar por nome, telefone ou email..." value={search} onChange={(e) => { setSearch(e.target.value); load(e.target.value, 1); }} className="pl-10" />
      </div>

      {error && <p className="text-destructive text-sm">{error}</p>}

      {loading ? (
        <div className="text-center py-12 text-muted-foreground">Carregando...</div>
      ) : (
        <div className="app-surface overflow-hidden">
          <DataTable
            columns={columns}
            data={patients}
            keyExtractor={(p) => p.id}
            emptyMessage="Nenhum paciente cadastrado. Clique em '+ Novo Paciente' para começar."
            page={page}
            total={total}
            perPage={perPage}
            onPageChange={(p) => load(search, p)}
          />
        </div>
      )}

      <Modal open={modalOpen} onClose={() => setModalOpen(false)} title={editId ? "Editar Paciente" : "Novo Paciente"} size="lg">
        <form onSubmit={handleSubmit(onSave)} className="space-y-4">
          <div>
            <Input label="Nome completo *" {...register("full_name")} />
            <FieldError message={errors.full_name?.message} />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Input label="Nº Prontuário" {...register("chart_number")} />
              <FieldError message={errors.chart_number?.message} />
            </div>
            <div>
              <Input label="Telefone" {...register("phone")} />
              <FieldError message={errors.phone?.message} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Input label="Email" type="email" {...register("email")} />
              <FieldError message={errors.email?.message} />
            </div>
            <Input label="Data de Nascimento" type="date" {...register("birth_date")} />
          </div>
          <Input label="Telefone de Emergência" {...register("emergency_phone")} />
          <TextArea label="Histórico de Saúde" rows={3} {...register("health_history")} />
          <TextArea label="Medicações em Uso" rows={3} {...register("medications_in_use")} />
          <TextArea label="Observações Administrativas" rows={3} {...register("admin_notes")} />
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="outline" type="button" onClick={() => setModalOpen(false)}>Cancelar</Button>
            <Button type="submit" disabled={saving}>{saving ? "Salvando..." : "Salvar"}</Button>
          </div>
        </form>
      </Modal>

      <ConfirmDialog open={confirmOpen} onClose={() => setConfirmOpen(false)} onConfirm={confirmAction} title={confirmTitle} message="Essa ação pode ser revertida depois." confirmLabel="Confirmar" />
      <ImportPatientsModal open={importOpen} onClose={() => setImportOpen(false)} onImported={() => load(search, 1)} />
    </div>
  );
}
