import { useEffect, useState } from "react";
import { ShieldCheck, Key, Database, Lock, Shield, AlertCircle } from "lucide-react";
import { api, type BackupConfigData } from "../../lib/api";
import { format } from "date-fns";
import Modal from "../ui/Modal";
import RecoveryCodeSetup from "./RecoveryCodeSetup";
import DataKeyRotation from "./DataKeyRotation";
import { refreshAuthState } from "../../lib/auth";

interface Props {
  onboardingCompleted: boolean;
  /**
   * A conta não tem segunda via da chave de dados.
   *
   * Concluir o onboarding NÃO garante isso: quem vem de versão anterior tem um
   * código válido mas nenhum envelope, porque um envelope só nasce do segredo em
   * claro. Mostrar "Ativo" nesse estado seria afirmar uma proteção que a conta
   * não tem.
   */
  recoveryWrapMissing?: boolean;
  /**
   * A chave dos prontuários ainda depende apenas desta máquina.
   *
   * É a pendência de segurança mais relevante do app: enquanto for verdade, quem
   * tiver acesso a esta conta do sistema operacional lê tudo sem saber a senha.
   */
  keyRotationPending?: boolean;
}

export default function SecurityStatusCard({
  onboardingCompleted,
  recoveryWrapMissing = false,
  keyRotationPending = false,
}: Props) {
  const [config, setConfig] = useState<BackupConfigData | null>(null);
  const [setupAberto, setSetupAberto] = useState(false);
  const [rotacaoAberta, setRotacaoAberta] = useState(false);
  // Sem booleano local de "já resolvi": o estado verdadeiro está no servidor, e
  // guardá-lo aqui fazia o cartão voltar a amarelo em qualquer remontagem.
  const faltaRotacao = keyRotationPending;
  const faltaCodigo = recoveryWrapMissing || !onboardingCompleted;

  useEffect(() => {
    const ctrl = new AbortController();
    api.backup.getConfig()
      .then((c) => { if (!ctrl.signal.aborted) setConfig(c); })
      .catch(() => {});
    return () => ctrl.abort();
  }, []);

  const items = [
    {
      icon: ShieldCheck,
      label: "Senha configurada",
      status: "ok" as const,
      color: "text-success",
      bgClass: "bg-success/10",
    },
    {
      icon: faltaCodigo ? AlertCircle : Key,
      label: "Código de recuperação",
      status: faltaCodigo ? ("warning" as const) : ("ok" as const),
      color: faltaCodigo ? "text-yellow-700" : "text-success",
      bgClass: faltaCodigo ? "bg-yellow-50" : "bg-success/10",
      action: faltaCodigo ? () => setSetupAberto(true) : undefined,
    },
    {
      icon: Database,
      label: "Último backup",
      status: "info" as const,
      detail: config?.last_backup_at
        ? format(new Date(config.last_backup_at), "dd/MM/yyyy")
        : "Nunca",
      color: config?.last_backup_at ? "text-foreground" : "text-muted-foreground",
      bgClass: "bg-muted",
    },
    {
      icon: config?.last_backup_at ? Lock : Shield,
      label: "Backup criptografado",
      status: config?.last_backup_at ? ("ok" as const) : ("muted" as const),
      detail: config?.last_backup_at ? "Ativo" : "Não criado",
      color: config?.last_backup_at ? "text-success" : "text-muted-foreground",
      bgClass: config?.last_backup_at ? "bg-success/10" : "bg-muted",
    },
    {
      icon: faltaRotacao ? AlertCircle : ShieldCheck,
      label: "Chave dos prontuários",
      status: faltaRotacao ? ("warning" as const) : ("ok" as const),
      detail: faltaRotacao ? undefined : "Ligada à senha",
      color: faltaRotacao ? "text-yellow-700" : "text-success",
      bgClass: faltaRotacao ? "bg-yellow-50" : "bg-success/10",
      action: faltaRotacao ? () => setRotacaoAberta(true) : undefined,
    },
  ];

  return (
    <>
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4">
        {items.map((item) => {
          const conteudo = (
            <>
              <div className="flex items-center gap-2 mb-2">
                <item.icon className={`h-4 w-4 ${item.color}`} aria-hidden="true" />
                <span className="text-xs font-medium text-muted-foreground">{item.label}</span>
              </div>
              <p className={`text-sm font-semibold ${item.color}`}>
                {item.status === "ok" && (item.detail || "Ativo")}
                {item.status === "warning" && "Configurar"}
                {item.status === "info" && (item.detail || "—")}
                {item.status === "muted" && (item.detail || "—")}
              </p>
            </>
          );

          const classes = `${item.bgClass} rounded-2xl p-4 border border-border/50 text-left`;

          // Pendências viram botão: um aviso que não dá para agir é só ruído, e
          // esta em particular tem consequência definitiva se for ignorada.
          return item.action ? (
            <button
              key={item.label}
              type="button"
              onClick={item.action}
              className={`${classes} cursor-pointer transition-shadow hover:shadow-md focus:outline-none focus:ring-2 focus:ring-teal-600 focus:ring-offset-2`}
            >
              {conteudo}
            </button>
          ) : (
            <div key={item.label} className={classes}>
              {conteudo}
            </div>
          );
        })}
      </div>

      <Modal
        open={rotacaoAberta}
        onClose={() => setRotacaoAberta(false)}
        title="Proteger a chave dos prontuários"
        size="md"
      >
        <DataKeyRotation
          onDone={() => {
            setRotacaoAberta(false);
            void refreshAuthState();
          }}
          onCancel={() => setRotacaoAberta(false)}
        />
      </Modal>

      <Modal
        open={setupAberto}
        onClose={() => setSetupAberto(false)}
        title="Segunda via da chave"
        size="md"
      >
        <RecoveryCodeSetup
          onDone={() => {
            setSetupAberto(false);
            void refreshAuthState();
          }}
          onCancel={() => setSetupAberto(false)}
        />
      </Modal>
    </>
  );
}
