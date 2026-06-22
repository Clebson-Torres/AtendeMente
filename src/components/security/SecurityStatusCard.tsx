import { useEffect, useState } from "react";
import { ShieldCheck, Key, Database, Lock, Shield, AlertCircle } from "lucide-react";
import { api, type BackupConfigData } from "../../lib/api";
import { format } from "date-fns";

interface Props {
  onboardingCompleted: boolean;
}

export default function SecurityStatusCard({ onboardingCompleted }: Props) {
  const [config, setConfig] = useState<BackupConfigData | null>(null);

  useEffect(() => {
    api.backup.getConfig()
      .then(setConfig)
      .catch(() => {});
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
      icon: onboardingCompleted ? Key : AlertCircle,
      label: "Código de recuperação",
      status: onboardingCompleted ? ("ok" as const) : ("warning" as const),
      color: onboardingCompleted ? "text-success" : "text-yellow-600",
      bgClass: onboardingCompleted ? "bg-success/10" : "bg-yellow-50",
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
  ];

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {items.map((item) => (
        <div key={item.label} className={`${item.bgClass} rounded-2xl p-4 border border-border/50`}>
          <div className="flex items-center gap-2 mb-2">
            <item.icon className={`h-4 w-4 ${item.color}`} />
            <span className="text-xs font-medium text-muted-foreground">{item.label}</span>
          </div>
          <p className={`text-sm font-semibold ${item.color}`}>
            {item.status === "ok" && "Ativo"}
            {item.status === "warning" && "Pendente"}
            {item.status === "info" && (item.detail || "—")}
            {item.status === "muted" && (item.detail || "—")}
          </p>
        </div>
      ))}
    </div>
  );
}
