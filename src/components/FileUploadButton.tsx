import { useState } from "react";
import { createPortal } from "react-dom";
import { useForm } from "react-hook-form";
import { api, type FileUploadRequest, type RecordFile } from "../lib/api";
import Button from "./ui/Button";
import Input from "./ui/Input";
import { toast } from "./ui/Toast";
import { Upload, File, X, AlertCircle, CheckCircle } from "lucide-react";

interface Props {
  appointmentId: string;
  patientId: string;
  onUploadComplete?: () => void;
}

export default function FileUploadButton({ appointmentId, patientId, onUploadComplete }: Props) {
  const [isOpen, setIsOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [kind, setKind] = useState<"session_attachment" | "payment_receipt">("session_attachment");
  const [paymentId, setPaymentId] = useState("");

  const { register, handleSubmit, reset, formState: { errors } } = useForm<{
    file_name: string;
    file_size: number;
    mime_type: string;
  }>();

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > 20 * 1024 * 1024) {
      toast("Arquivo excede 20 MB.", "error");
      return;
    }
    setSelectedFile(file);
    reset({
      file_name: file.name,
      file_size: file.size,
      mime_type: file.type || "application/octet-stream",
    });
  }

  async function onSubmit(data: { file_name: string; file_size: number; mime_type: string }) {
    if (!selectedFile) return;
    setLoading(true);
    try {
      const session = await api.files.uploadSession({
        appointment_id: appointmentId,
        patient_id: patientId,
        payment_id: kind === "payment_receipt" ? paymentId : undefined,
        kind,
        file_name: data.file_name,
        file_size: data.file_size,
        mime_type: data.mime_type,
      });

      // Upload content
      await api.files.uploadContent(session.file_id, selectedFile);

      // Confirm
      await api.files.confirm(session.file_id);

      toast("Arquivo enviado com sucesso.");
      setIsOpen(false);
      setSelectedFile(null);
      onUploadComplete?.();
    } catch (e: any) {
      toast(e.message || "Erro ao enviar arquivo", "error");
    } finally {
      setLoading(false);
    }
  }

  if (!isOpen) {
    return (
      <Button variant="outline" size="sm" onClick={() => setIsOpen(true)}>
        <Upload className="h-4 w-4 mr-2" /> Anexar Arquivo
      </Button>
    );
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={() => setIsOpen(false)}>
      <div onClick={(e) => e.stopPropagation()} className="bg-white rounded-xl p-6 w-full max-w-md animate-fade-in max-h-[85vh] overflow-y-auto m-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-lg">Anexar Arquivo</h3>
          <button onClick={() => setIsOpen(false)} className="text-muted-foreground hover:text-foreground">
            <X className="h-5 w-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1">Arquivo</label>
            <input
              type="file"
              accept=".pdf,.png,.jpg,.jpeg,.doc,.docx"
              onChange={handleFileChange}
              required
              disabled={loading}
              className="w-full text-sm"
            />
            {selectedFile && (
              <p className="text-xs text-success mt-1 flex items-center gap-1">
                <CheckCircle className="h-3 w-3" /> {selectedFile.name} ({(selectedFile.size / 1024).toFixed(1)} KB)
              </p>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium mb-1">Tipo</label>
            <select value={kind} onChange={(e) => setKind(e.target.value as any)} className="w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm">
              <option value="session_attachment">Anexo da Sessão</option>
              <option value="payment_receipt">Recibo de Pagamento</option>
            </select>
          </div>

          {kind === "payment_receipt" && (
            <div>
              <Input label="ID do Pagamento (obrigatório)" value={paymentId} onChange={(e) => setPaymentId(e.target.value)} />
            </div>
          )}

          <div className="flex gap-2 justify-end pt-2">
            <Button type="button" variant="outline" onClick={() => setIsOpen(false)}>Cancelar</Button>
            <Button type="submit" disabled={loading || !selectedFile}>
              {loading ? "Enviando..." : "Enviar"}
            </Button>
          </div>
        </form>
      </div>
    </div>,
    document.body
  );
}