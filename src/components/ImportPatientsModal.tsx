import { useState, useRef } from "react";
import { api } from "../lib/api";
import Button from "./ui/Button";
import Modal from "./ui/Modal";
import DataTable from "./ui/DataTable";
import type { Column } from "./ui/DataTable";
import { toast } from "./ui/Toast";
import { Upload, FileSpreadsheet, AlertCircle, CheckCircle2 } from "lucide-react";

interface CsvRow {
  line: number;
  full_name: string;
  chart_number: string | null;
  phone: string | null;
  email: string | null;
  birth_date: string | null;
  health_history: string | null;
  medications_in_use: string | null;
  emergency_phone: string | null;
  admin_notes: string | null;
  errors: string[];
}

interface ImportPreview {
  total_rows: number;
  valid_rows: number;
  error_rows: number;
  rows: CsvRow[];
}

interface Props {
  open: boolean;
  onClose: () => void;
  onImported: () => void;
}

export default function ImportPatientsModal({ open, onClose, onImported }: Props) {
  const fileRef = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [sessionId, setSessionId] = useState("");
  const [importing, setImporting] = useState(false);
  const [parsing, setParsing] = useState(false);
  const [done, setDone] = useState(false);
  const [importedCount, setImportedCount] = useState(0);

  function reset() {
    setPreview(null);
    setSessionId("");
    setImporting(false);
    setParsing(false);
    setDone(false);
    setImportedCount(0);
    if (fileRef.current) fileRef.current.value = "";
  }

  function handleClose() {
    reset();
    onClose();
  }

  async function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;

    if (!file.name.endsWith(".csv")) {
      toast("Selecione um arquivo CSV.", "error");
      return;
    }

    setParsing(true);
    setPreview(null);

    try {
      const text = await file.text();
      const bytes = new TextEncoder().encode(text);
      const base64 = btoa(bytes.reduce((s, b) => s + String.fromCharCode(b), ""));
      const result = await api.patients.importPreview(base64);
      setPreview(result.preview);
      setSessionId(result.session_id);
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : "Erro ao importar";
      toast(message, "error");
    } finally {
      setParsing(false);
    }
  }

  async function handleImport() {
    if (!preview || !sessionId) return;
    const validRows = preview.rows.filter((r) => r.errors.length === 0);
    if (validRows.length === 0) return;

    setImporting(true);
    try {
      const result = await api.patients.importCommit(sessionId, validRows);
      setImportedCount(result.imported);
      setDone(true);
      toast(`${result.imported} paciente(s) importado(s) com sucesso.`);
      onImported();
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : "Erro ao importar";
      toast(message, "error");
    } finally {
      setImporting(false);
    }
  }

  const columns: Column<CsvRow>[] = [
    { key: "line", header: "#", className: "w-10 text-center" },
    {
      key: "full_name", header: "Nome", className: "font-medium",
      render: (r) => (
        <span className={r.errors.length > 0 ? "text-destructive" : ""}>
          {r.full_name || <span className="italic text-muted-foreground">(vazio)</span>}
        </span>
      ),
    },
    { key: "phone", header: "Telefone", render: (r) => r.phone ?? "-" },
    { key: "email", header: "Email", render: (r) => r.email ?? "-" },
    { key: "birth_date", header: "Nascimento", render: (r) => r.birth_date ?? "-" },
    {
      key: "errors", header: "",
      render: (r) => r.errors.length > 0 ? (
        <span className="text-destructive text-xs" title={r.errors.join("; ")}>
          <AlertCircle className="h-4 w-4 inline mr-1" />{r.errors[0]}
        </span>
      ) : null,
    },
  ];

  return (
    <Modal open={open} onClose={handleClose} title="Importar Pacientes (CSV)" size="lg">
      {!preview && !parsing && !done && (
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Selecione um arquivo CSV com as colunas: <code>full_name</code> (obrigatório), <code>phone</code>, <code>email</code>, <code>birth_date</code>, <code>chart_number</code>, <code>health_history</code>, <code>medications_in_use</code>, <code>emergency_phone</code>, <code>admin_notes</code>.
          </p>
          <p className="text-xs text-muted-foreground">
            Colunas em português também são aceitas: <code>nome</code>, <code>telefone</code>, <code>nascimento</code>, etc.
          </p>
          <div className="border-2 border-dashed border-border rounded-xl p-8 text-center">
            <input ref={fileRef} type="file" accept=".csv" onChange={handleFileChange} className="hidden" />
            <Button onClick={() => fileRef.current?.click()} variant="outline">
              <Upload className="h-4 w-4 mr-2" /> Selecionar Arquivo CSV
            </Button>
          </div>
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="outline" onClick={handleClose}>Cancelar</Button>
          </div>
        </div>
      )}

      {parsing && (
        <div className="text-center py-8">
          <FileSpreadsheet className="h-8 w-8 mx-auto text-primary animate-pulse mb-2" />
          <p className="text-muted-foreground">Analisando arquivo...</p>
        </div>
      )}

      {preview && !done && (
        <div className="space-y-4">
          <div className="flex items-center gap-4 text-sm">
            <span className="text-muted-foreground">Total: <strong>{preview.total_rows}</strong></span>
            <span className="text-green-600">Válidos: <strong>{preview.valid_rows}</strong></span>
            {preview.error_rows > 0 && (
              <span className="text-destructive">Com erros: <strong>{preview.error_rows}</strong></span>
            )}
          </div>
          <div className="max-h-64 overflow-y-auto border border-border rounded-xl">
            <DataTable
              columns={columns}
              data={preview.rows}
              keyExtractor={(r) => String(r.line)}
              emptyMessage="Nenhum dado encontrado."
            />
          </div>
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="outline" onClick={handleClose}>Cancelar</Button>
            <Button onClick={handleImport} disabled={importing || preview.valid_rows === 0}>
              {importing ? "Importando..." : `Importar ${preview.valid_rows} Paciente(s)`}
            </Button>
          </div>
        </div>
      )}

      {done && (
        <div className="text-center py-8 space-y-4">
          <CheckCircle2 className="h-12 w-12 mx-auto text-green-500" />
          <p className="text-lg font-medium">{importedCount} paciente(s) importado(s) com sucesso.</p>
          <Button onClick={handleClose}>Concluir</Button>
        </div>
      )}
    </Modal>
  );
}
