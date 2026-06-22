import { useState } from "react";
import { ShieldAlert, Copy, Check, Download } from "lucide-react";
import { toast } from "../ui/Toast";
import { downloadFile } from "../../lib/utils";

interface Props {
  userId: string;
  secret: string;
  onNext: () => void;
}

export default function RecoverySecretScreen({ userId, secret, onNext }: Props) {
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(secret);
      setCopied(true);
      toast("Código copiado!", "success");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast("Erro ao copiar", "error");
    }
  }

  function handleDownload() {
    const content = JSON.stringify({ version: 1, user_id: userId, recovery_secret: secret }, null, 2);
    const blob = new Blob([content], { type: "application/json" });
    downloadFile(blob, `atendemente-recovery-${userId.slice(0, 8)}.json`);
    toast("Arquivo baixado!", "success");
  }

  return (
    <div className="app-surface w-full max-w-md p-8 space-y-6 animate-fade-in">
      <h1 className="text-2xl font-display font-semibold text-slate-900 text-center">Chave de Recuperação</h1>

      <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-xl">
        <div className="flex gap-3">
          <ShieldAlert className="h-5 w-5 text-yellow-600 flex-shrink-0 mt-0.5" />
          <div>
            <p className="text-yellow-800 font-medium">Salve esta chave em um local seguro!</p>
            <p className="text-yellow-700 text-sm mt-1">
              Sem este código, <strong>não será possível recuperar sua senha</strong> caso você a esqueça.
              Ele é único e não pode ser regenerado.
            </p>
          </div>
        </div>
      </div>

      <div className="bg-muted p-5 rounded-2xl border border-border">
        <p className="text-2xl font-mono tracking-[0.25em] text-center text-slate-900 select-all">
          {secret}
        </p>
      </div>

      <div className="flex gap-3">
        <button onClick={handleCopy} className="flex-1 flex items-center justify-center gap-2 border border-border bg-white text-foreground py-2.5 rounded-xl hover:bg-accent font-medium transition-colors cursor-pointer">
          {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
          {copied ? "Copiado" : "Copiar"}
        </button>
        <button onClick={handleDownload} className="flex-1 flex items-center justify-center gap-2 bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium transition-colors cursor-pointer">
          <Download className="h-4 w-4" /> Baixar
        </button>
      </div>

      <p className="text-xs text-muted-foreground text-center">
        Você só verá este código uma vez. Baixe o arquivo ou copie para um local seguro antes de continuar.
      </p>

      <label className="flex items-start gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={saved}
          onChange={(e) => setSaved(e.target.checked)}
          className="mt-1 h-4 w-4 rounded border-border text-primary focus:ring-primary cursor-pointer"
        />
        <span className="text-sm text-foreground">
          Salvei meu código de recuperação em um local seguro
        </span>
      </label>

      <button
        onClick={onNext}
        disabled={!saved}
        className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors cursor-pointer"
      >
        Continuar
      </button>
    </div>
  );
}
