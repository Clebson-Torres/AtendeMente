import { useEffect, type ReactNode } from "react";
import { cn } from "../../lib/utils";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
}

const widths = {
  sm: "max-w-sm",
  md: "max-w-lg",
  lg: "max-w-2xl",
  xl: "max-w-4xl",
};

export default function Modal({ open, onClose, title, children, size = "md" }: Props) {
  useEffect(() => {
    if (open) document.body.style.overflow = "hidden";
    else document.body.style.overflow = "";
    const handleEsc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    if (open) window.addEventListener("keydown", handleEsc);
    return () => {
      document.body.style.overflow = "";
      window.removeEventListener("keydown", handleEsc);
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" onClick={onClose}>
      <div className="fixed inset-0 bg-black/40 backdrop-blur-sm" />
      <div
        className={cn(
          "relative app-surface animate-fade-in max-h-[90vh] m-4",
          widths[size],
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="overflow-auto max-h-[90vh] [scrollbar-gutter:stable] rounded-[28px]">
          <div className="flex items-center justify-between px-6 pt-6 pb-2">
            <h2 className="text-lg font-semibold text-slate-900">{title}</h2>
            <button onClick={onClose} aria-label="Fechar" className="text-muted-foreground hover:text-foreground transition-colors rounded-full h-8 w-8 flex items-center justify-center hover:bg-secondary"><X className="h-5 w-5" /></button>
          </div>
          <div className="p-6">{children}</div>
        </div>
      </div>
    </div>
  );
}
