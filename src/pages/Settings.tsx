import { useEffect, useRef, useState } from "react";
import { api, type BackupConfigData } from "../lib/api";
import { Shield, Download, Upload, RefreshCw, Lock, Unlock, Smartphone, Wifi, WifiOff } from "lucide-react";
import Button from "../components/ui/Button";
import Modal from "../components/ui/Modal";
import Select from "../components/ui/Select";
import { downloadFile } from "../lib/utils";
import { toast } from "../components/ui/Toast";

export default function Settings() {
  const [config, setConfig] = useState<BackupConfigData | null>(null);
  const [mobileAccess, setMobileAccess] = useState(false);
  const [mobileAccessLoading, setMobileAccessLoading] = useState(true);
  const [mobileAccessSaving, setMobileAccessSaving] = useState(false);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [autoEnabled, setAutoEnabled] = useState(false);
  const [frequency, setFrequency] = useState("daily");
  const [saving, setSaving] = useState(false);
  const [showPasswordModal, setShowPasswordModal] = useState(false);
  const [passwordMode, setPasswordMode] = useState<"export" | "import">("export");
  const [backupPassword, setBackupPassword] = useState("");
  const [backupPasswordConfirm, setBackupPasswordConfirm] = useState("");
  const [passwordError, setPasswordError] = useState("");
  const [passwordForRestore, setPasswordForRestore] = useState<string | null>(null);
  const pendingRestoreFile = useRef<Uint8Array | null>(null);

  useEffect(() => {
    const ctrl = new AbortController();
    Promise.all([
      api.backup.getConfig(),
      api.settings.getMobileAccess(),
    ])
      .then(([c, m]) => {
        if (!ctrl.signal.aborted) {
          setConfig(c);
          setAutoEnabled(c.frequency !== "never");
          setFrequency(c.frequency !== "never" ? c.frequency : "daily");
          setMobileAccess(m.enabled);
        }
      })
      .catch(() => {})
      .finally(() => { if (!ctrl.signal.aborted) { setMobileAccessLoading(false); setLoading(false); } });
    return () => ctrl.abort();
  }, []);

  const handleCreateBackupClick = () => {
    setPasswordMode("export");
    setBackupPassword("");
    setBackupPasswordConfirm("");
    setPasswordError("");
    setShowPasswordModal(true);
  };

  const handleConfirmExport = async () => {
    const pw = backupPassword;
    if (pw.length < 12) {
      setPasswordError("Minimo 12 caracteres para backup com senha.");
      return;
    }
    if (pw !== backupPasswordConfirm) {
      setPasswordError("Senhas nao conferem.");
      return;
    }
    setShowPasswordModal(false);
    setCreating(true);
    try {
      const password = pw;
      const { blob, fileName } = await api.backup.create(password);
      await downloadFile(blob, fileName);
      const c = await api.backup.getConfig();
      setConfig(c);
      toast("Backup baixado com sucesso.");
    } catch (e: any) {
      toast(e.message || "Erro ao criar backup", "error");
    } finally {
      setCreating(false);
    }
  };

  const handleRestoreBackup = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".zip,.atendemente";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const isEncrypted = file.name.endsWith(".atendemente") || file.name.endsWith(".atendemente");
      if (isEncrypted) {
        const buffer = await file.arrayBuffer();
        pendingRestoreFile.current = new Uint8Array(buffer);
        setPasswordMode("import");
        setBackupPassword("");
        setBackupPasswordConfirm("");
        setPasswordError("");
        setShowPasswordModal(true);
        return;
      }
      await doRestore(new Uint8Array(await file.arrayBuffer()), undefined);
    };
    input.click();
  };

  const handleConfirmRestore = async () => {
    const pw = backupPassword;
    if (pw.length < 12) {
      setPasswordError("Minimo 12 caracteres.");
      return;
    }
    setShowPasswordModal(false);
    if (pendingRestoreFile.current) {
      await doRestore(pendingRestoreFile.current, pw);
      pendingRestoreFile.current = null;
    }
  };

  const doRestore = async (bytes: Uint8Array, password: string | undefined) => {
    setRestoring(true);
    try {
      let binary = "";
      for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
      const base64 = btoa(binary);
      await api.backup.restore(base64, password);
      toast("Backup restaurado. Recarregue a pagina.");
    } catch (e: any) {
      toast(e.message || "Erro ao restaurar backup", "error");
    } finally {
      setRestoring(false);
    }
  };

  const handleSaveAutoConfig = async () => {
    setSaving(true);
    try {
      const freq = autoEnabled ? frequency : "never";
      await api.backup.setConfig(freq);
      setConfig((prev) => prev ? { ...prev, frequency: freq } : null);
      toast("Configuracao salva.");
    } catch (e: any) {
      toast(e.message || "Erro ao salvar", "error");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="p-6 max-w-2xl mx-auto space-y-4">
        <div className="h-8 w-48 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
        <div className="h-32 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
      </div>
    );
  }

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6">
      <div className="flex items-center gap-3">
        <Shield className="h-6 w-6 text-primary" />
        <h1 className="font-display text-2xl font-semibold">Configurações</h1>
      </div>

      <div className="app-surface p-5 space-y-5">
        <h2 className="font-display text-lg font-semibold flex items-center gap-2">
          <Download className="h-5 w-5" />
          Backup Manual
        </h2>
        <p className="text-sm text-muted-foreground">
          Defina uma senha para criptografar o backup (min. 12 caracteres).
          O backup sera criptografado com AES-256-GCM.
        </p>
        <Button onClick={handleCreateBackupClick} disabled={creating}>
          {creating ? <RefreshCw className="h-4 w-4 animate-spin mr-2" /> : <Lock className="h-4 w-4 mr-2" />}
          {creating ? "Criando..." : "Exportar Backup"}
        </Button>
        {config?.last_backup_at && (
          <p className="text-xs text-muted-foreground">
            Ultimo backup: {new Date(config.last_backup_at).toLocaleString("pt-BR")}
          </p>
        )}
      </div>

      <div className="app-surface p-5 space-y-5">
        <h2 className="font-display text-lg font-semibold flex items-center gap-2">
          <Upload className="h-5 w-5" />
          Restaurar Backup
        </h2>
        <p className="text-sm text-muted-foreground">
          Selecione um arquivo ZIP de backup para restaurar todos os dados.
          Esta operacao substituira os dados atuais.
        </p>
        <Button variant="destructive" onClick={handleRestoreBackup} disabled={restoring}>
          {restoring ? <RefreshCw className="h-4 w-4 animate-spin mr-2" /> : <Upload className="h-4 w-4 mr-2" />}
          {restoring ? "Restaurando..." : "Selecionar ZIP e Restaurar"}
        </Button>
      </div>

      <div className="app-surface p-5 space-y-5">
        <h2 className="font-display text-lg font-semibold flex items-center gap-2">
          <RefreshCw className="h-5 w-5" />
          Backup Automatico
        </h2>
        <p className="text-sm text-muted-foreground">
          Configure a frequencia de backup automatico. O backup sera gerado
          em segundo plano enquanto o servidor estiver ativo.
        </p>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              role="switch"
              checked={autoEnabled}
              onChange={(e) => setAutoEnabled(e.target.checked)}
              className="h-5 w-9 appearance-none rounded-full bg-gray-300 dark:bg-gray-600 checked:bg-primary transition-colors duration-200 relative cursor-pointer
                before:absolute before:h-4 before:w-4 before:bg-white before:rounded-full before:top-0.5 before:left-0.5 before:transition-transform before:duration-200
                checked:before:translate-x-4"
            />
            <span className="text-sm font-medium">{autoEnabled ? "Ativado" : "Desativado"}</span>
          </label>
        </div>
        {autoEnabled && (
          <div className="flex items-center gap-3">
            <div className="flex-1">
              <Select
                value={frequency}
                onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setFrequency(e.target.value)}
                options={[
                  { value: "daily", label: "Diario" },
                  { value: "weekly", label: "Semanal" },
                ]}
              />
            </div>
            <Button onClick={handleSaveAutoConfig} disabled={saving}>
              {saving ? "Salvando..." : "Salvar"}
            </Button>
          </div>
        )}
        {autoEnabled && config?.last_backup_at && (
          <p className="text-xs text-muted-foreground">
            Ultimo backup: {new Date(config.last_backup_at).toLocaleString("pt-BR")}
          </p>
        )}
      </div>

      <div className="app-surface p-5 space-y-5">
        <h2 className="font-display text-lg font-semibold flex items-center gap-2">
          <Smartphone className="h-5 w-5" />
          Acesso Mobile
        </h2>
        <p className="text-sm text-muted-foreground">
          Permite acessar o AtendeMete pelo celular ou outros dispositivos na rede local.
        </p>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              role="switch"
              checked={mobileAccess}
              disabled={mobileAccessLoading || mobileAccessSaving}
              onChange={async (e) => {
                const enabled = e.target.checked;
                setMobileAccess(enabled);
                setMobileAccessSaving(true);
                try {
                  await api.settings.setMobileAccess(enabled);
                  toast(enabled ? "Acesso mobile ativado. Reinicie o aplicativo para aplicar." : "Acesso mobile desativado. Reinicie o aplicativo para aplicar.");
                } catch (err: any) {
                  setMobileAccess(!enabled);
                  toast(err.message || "Erro ao salvar", "error");
                } finally {
                  setMobileAccessSaving(false);
                }
              }}
              className="h-5 w-9 appearance-none rounded-full bg-gray-300 dark:bg-gray-600 checked:bg-primary transition-colors duration-200 relative cursor-pointer
                before:absolute before:h-4 before:w-4 before:bg-white before:rounded-full before:top-0.5 before:left-0.5 before:transition-transform before:duration-200
                checked:before:translate-x-4"
            />
            {mobileAccess ? <Wifi className="h-4 w-4 text-green-600" /> : <WifiOff className="h-4 w-4 text-muted-foreground" />}
            <span className="text-sm font-medium">{mobileAccess ? "Ativado" : "Desativado"}</span>
          </label>
          {mobileAccessSaving && <RefreshCw className="h-4 w-4 animate-spin text-muted-foreground" />}
        </div>
      </div>

      <Modal
        open={showPasswordModal}
        onClose={() => setShowPasswordModal(false)}
        title={passwordMode === "export" ? "Proteger Backup com Senha" : "Senha do Backup"}
        size="sm"
      >
        <p className="text-sm text-muted-foreground mb-4">
          {passwordMode === "export"
            ? "Defina uma senha de minimo 12 caracteres para criptografar o backup."
            : "Este backup esta criptografado. Informe a senha definida na exportacao."}
        </p>
        <div className="space-y-3">
          <input
            type="password"
            placeholder="Senha (min. 12 caracteres)"
            value={backupPassword}
            onChange={(e) => setBackupPassword(e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-input bg-background text-sm"
            autoFocus
          />
          {passwordMode === "export" && (
            <input
              type="password"
              placeholder="Confirmar senha"
              value={backupPasswordConfirm}
              onChange={(e) => setBackupPasswordConfirm(e.target.value)}
              className="w-full px-3 py-2 rounded-lg border border-input bg-background text-sm"
            />
          )}
        </div>
        {passwordError && <p className="text-sm text-destructive mt-2">{passwordError}</p>}
        <div className="flex gap-3 justify-end mt-4">
          <Button variant="ghost" onClick={() => setShowPasswordModal(false)}>
            Cancelar
          </Button>
          <Button
            onClick={passwordMode === "export" ? handleConfirmExport : handleConfirmRestore}
            disabled={passwordMode === "export" ? backupPassword.length < 12 || backupPassword !== backupPasswordConfirm : backupPassword.length < 12}
          >
            {passwordMode === "export" ? "Exportar" : "Restaurar"}
          </Button>
        </div>
      </Modal>
    </div>
  );
}
