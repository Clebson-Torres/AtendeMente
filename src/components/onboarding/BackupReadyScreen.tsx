import { useState } from "react";
import { Database, SkipForward, CheckCircle2, Lock } from "lucide-react";
import { api } from "../../lib/api";
import { toast } from "../ui/Toast";
import { downloadFile } from "../../lib/utils";

interface Props {
  onComplete: () => void;
  onSkip: () => void;
}

export default function BackupReadyScreen({ onComplete, onSkip }: Props) {
  const [choiceMade, setChoiceMade] = useState(false);
  const [showPasswordModal, setShowPasswordModal] = useState(false);
  const [backupPassword, setBackupPassword] = useState("");
  const [backupLoading, setBackupLoading] = useState(false);
  const [backupError, setBackupError] = useState("");

  async function handleCreateBackup() {
    if (backupPassword.length < 12) {
      setBackupError("A senha deve ter no mínimo 12 caracteres");
      return;
    }
    setBackupLoading(true);
    setBackupError("");
    try {
      const { blob, fileName } = await api.backup.create(backupPassword);
      downloadFile(blob, fileName);
      toast("Backup criado com sucesso!", "success");
      setShowPasswordModal(false);
      setChoiceMade(true);
    } catch (err: any) {
      setBackupError(err.message || "Erro ao criar backup");
    } finally {
      setBackupLoading(false);
    }
  }

  function handleSkip() {
    onSkip();
    setChoiceMade(true);
  }

  if (choiceMade) {
    return (
      <div className="app-surface w-full max-w-md p-8 space-y-6 animate-fade-in text-center">
        <div className="mx-auto h-16 w-16 rounded-full bg-success/10 flex items-center justify-center">
          <CheckCircle2 className="h-8 w-8 text-success" />
        </div>
        <h1 className="text-2xl font-display font-semibold text-slate-900">Tudo pronto!</h1>
        <p className="text-muted-foreground text-sm">
          Sua conta está configurada e protegida. Aproveite o AtendeMente!
        </p>
        <button onClick={onComplete} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium transition-colors cursor-pointer">
          Entrar no AtendeMente
        </button>
      </div>
    );
  }

  return (
    <>
      <div className="app-surface w-full max-w-md p-8 space-y-6 animate-fade-in">
        <h1 className="text-2xl font-display font-semibold text-slate-900 text-center">Backup de Segurança</h1>

        <p className="text-muted-foreground text-sm text-center">
          Crie um backup criptografado dos seus dados. Você pode restaurá-lo a qualquer momento.
        </p>

        <div className="space-y-3">
          <div className="bg-muted p-4 rounded-2xl border border-border flex items-start gap-3">
            <Lock className="h-5 w-5 text-primary flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-sm font-medium text-foreground">Criptografia de ponta a ponta</p>
              <p className="text-xs text-muted-foreground">Seus dados são protegidos com AES-256-GCM</p>
            </div>
          </div>
          <div className="bg-muted p-4 rounded-2xl border border-border flex items-start gap-3">
            <Database className="h-5 w-5 text-primary flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-sm font-medium text-foreground">Restauração completa</p>
              <p className="text-xs text-muted-foreground">Recupere todos os pacientes, agendamentos e registros</p>
            </div>
          </div>
        </div>

        <div className="flex gap-3">
          <button onClick={() => setShowPasswordModal(true)} className="flex-1 flex items-center justify-center gap-2 bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium transition-colors cursor-pointer">
            <Database className="h-4 w-4" /> Criar Backup
          </button>
          <button onClick={handleSkip} className="flex-1 flex items-center justify-center gap-2 border border-border bg-white text-foreground py-2.5 rounded-xl hover:bg-accent font-medium transition-colors cursor-pointer">
            <SkipForward className="h-4 w-4" /> Pular
          </button>
        </div>
      </div>

      {showPasswordModal && (
        <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center p-4" onClick={() => setShowPasswordModal(false)}>
          <div className="app-surface w-full max-w-sm p-6 space-y-4 animate-fade-in" onClick={(e) => e.stopPropagation()}>
            <h2 className="text-lg font-display font-semibold text-slate-900">Proteger Backup</h2>
            <p className="text-sm text-muted-foreground">Defina uma senha para criptografar o backup (mínimo 12 caracteres).</p>
            {backupError && <p className="text-destructive text-sm bg-destructive/10 p-2 rounded-lg">{backupError}</p>}
            <input
              type="password"
              placeholder="Senha do backup"
              value={backupPassword}
              onChange={(e) => setBackupPassword(e.target.value)}
              className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1"
              autoFocus
            />
            <div className="flex gap-3 pt-2">
              <button onClick={() => setShowPasswordModal(false)} className="flex-1 border border-border bg-white text-foreground py-2 rounded-xl hover:bg-accent font-medium text-sm transition-colors cursor-pointer">
                Cancelar
              </button>
              <button onClick={handleCreateBackup} disabled={backupLoading} className="flex-1 bg-primary text-primary-foreground py-2 rounded-xl hover:bg-primary/90 font-medium text-sm disabled:opacity-50 transition-colors cursor-pointer">
                {backupLoading ? "Criando..." : "Criar Backup"}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
