import { useEffect, useState } from "react";
import { api, type RecordFile } from "../lib/api";
import Button from "./ui/Button";
import { Download, FileText, FileImage, FileIcon } from "lucide-react";

interface Props {
  appointmentId: string;
  onRefresh?: () => void;
}

export default function FileList({ appointmentId, onRefresh }: Props) {
  const [files, setFiles] = useState<RecordFile[]>([]);
  const [loading, setLoading] = useState(true);

  async function load() {
    setLoading(true);
    try {
      const data = await api.files.list(appointmentId);
      setFiles(data);
    } catch {
      setFiles([]);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, [appointmentId]);

  function getFileIcon(mimeType: string) {
    if (mimeType.startsWith("image/")) return <FileImage className="h-4 w-4 text-green-600" />;
    if (mimeType === "application/pdf") return <FileText className="h-4 w-4 text-red-600" />;
    if (mimeType.includes("word") || mimeType.includes("document")) return <FileIcon className="h-4 w-4 text-blue-600" />;
    return <FileIcon className="h-4 w-4 text-gray-500" />;
  }

  function formatSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function handleDownload(file: RecordFile) {
    try {
      const { blob, fileName } = await api.files.download(file.id);
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = fileName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e: any) {
      console.error("Download failed:", e);
    }
  }

  if (loading) return <div className="text-muted-foreground text-sm py-4">Carregando arquivos...</div>;

  if (files.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground border-2 border-dashed border-border rounded-xl">
        <p className="text-sm">Nenhum arquivo anexado.</p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {files.map((f) => (
        <div
          key={f.id}
          className="flex items-center justify-between p-3 bg-muted/30 rounded-xl border border-border"
        >
          <div className="flex items-center gap-3">
            {getFileIcon(f.mime_type)}
            <div>
              <p className="font-medium text-sm">{f.original_name}</p>
              <p className="text-xs text-muted-foreground">
                {f.kind === "payment_receipt" ? "Recibo" : "Anexo"} • {formatSize(f.byte_size)} • {f.uploaded_at.split("T")[0]}
              </p>
            </div>
          </div>
          <button
            onClick={() => handleDownload(f)}
            className="text-primary hover:text-primary/80 text-sm flex items-center gap-1"
          >
            <Download className="h-4 w-4" /> Baixar
          </button>
        </div>
      ))}
    </div>
  );
}