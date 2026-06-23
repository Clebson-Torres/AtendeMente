import { cn } from "../../lib/utils";

const variants: Record<string, string> = {
  active: "bg-success/10 text-success border-transparent",
  inactive: "bg-muted text-muted-foreground border-transparent",
  scheduled: "bg-blue-100 text-blue-800 border-transparent",
  confirmed: "bg-accent text-accent-foreground border-transparent",
  unconfirmed: "bg-yellow-100 text-yellow-800 border-transparent",
  pending: "bg-yellow-100 text-yellow-800 border-transparent",
  cancelled: "bg-destructive/10 text-destructive border-transparent",
  completed: "bg-success/10 text-success border-transparent",
  no_show: "bg-gray-100 text-gray-600 border-transparent",
  paid: "bg-success/10 text-success border-transparent",
  unpaid: "bg-yellow-100 text-yellow-800 border-transparent",
  partial: "bg-orange-100 text-orange-800 border-transparent",
};

const labels: Record<string, string> = {
  active: "Ativo",
  inactive: "Inativo",
  scheduled: "Agendado",
  confirmed: "Confirmado",
  unconfirmed: "Nao confirmado",
  pending: "Pendente",
  cancelled: "Cancelado",
  completed: "Concluido",
  no_show: "Nao compareceu",
  paid: "Pago",
  unpaid: "Nao pago",
  partial: "Parcial",
};

interface Props {
  status: string;
  className?: string;
  outline?: boolean;
}

export default function StatusBadge({ status, className, outline }: Props) {
  const style = variants[status.toLowerCase()] || "bg-muted text-muted-foreground border-transparent";
  const label = labels[status.toLowerCase()] || status;
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium border",
        outline ? "bg-transparent border-border text-muted-foreground" : style,
        className,
      )}
    >
      {label}
    </span>
  );
}
