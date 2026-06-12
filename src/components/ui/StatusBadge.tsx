import { cn } from "../../lib/utils";

const variants: Record<string, string> = {
  active: "bg-success/10 text-success border-transparent",
  inactive: "bg-muted text-muted-foreground border-transparent",
  confirmed: "bg-accent text-accent-foreground border-transparent",
  pending: "bg-yellow-100 text-yellow-800 border-transparent",
  cancelled: "bg-destructive/10 text-destructive border-transparent",
  completed: "bg-success/10 text-success border-transparent",
  paid: "bg-success/10 text-success border-transparent",
  unpaid: "bg-yellow-100 text-yellow-800 border-transparent",
  partial: "bg-orange-100 text-orange-800 border-transparent",
};

interface Props {
  status: string;
  className?: string;
  outline?: boolean;
}

export default function StatusBadge({ status, className, outline }: Props) {
  const style = variants[status.toLowerCase()] || "bg-muted text-muted-foreground border-transparent";
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium border",
        outline ? "bg-transparent border-border text-muted-foreground" : style,
        className,
      )}
    >
      {status}
    </span>
  );
}
