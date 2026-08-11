import { useEffect, useRef, useCallback, type ReactNode } from "react";
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

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )
  );
}

export default function Modal({ open, onClose, title, children, size = "md" }: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<Element | null>(null);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key !== "Tab" || !dialogRef.current) return;
      const focusable = getFocusableElements(dialogRef.current);
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    },
    [onClose]
  );

  // Foco e trava de scroll: SOMENTE quando `open` muda.
  //
  // Antes isto dividia um efeito com o listener de teclado, cujas dependencias
  // incluem `handleKeyDown` -> `onClose`. Como todo chamador passa uma arrow
  // inline (`onClose={() => setX(false)}`), a identidade muda a cada render: cada
  // tecla digitada dentro do modal re-rodava o efeito e o
  // `dialogRef.current.focus()` puxava o foco do input de volta para o container.
  // Sintoma: o campo aceitava um caractere e parava de registrar.
  useEffect(() => {
    if (!open) return;
    previousActiveElement.current = document.activeElement;
    document.body.style.overflow = "hidden";
    const timer = setTimeout(() => dialogRef.current?.focus(), 0);
    return () => {
      clearTimeout(timer);
      document.body.style.overflow = "";
      // Devolve o foco a quem abriu o modal.
      if (previousActiveElement.current instanceof HTMLElement) {
        previousActiveElement.current.focus();
      }
    };
  }, [open]);

  // Teclado em efeito separado: re-registrar o listener e barato e nao mexe no foco.
  useEffect(() => {
    if (!open) return;
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, handleKeyDown]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
    >
      <div className="fixed inset-0 bg-black/40 backdrop-blur-sm" aria-hidden="true" />
      <div
        ref={dialogRef}
        tabIndex={-1}
        className={cn(
          "relative app-surface animate-fade-in max-h-[90vh] m-4 outline-none",
          widths[size],
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="overflow-auto max-h-[90vh] [scrollbar-gutter:stable] rounded-[28px]">
          <div className="flex items-center justify-between px-6 pt-6 pb-2">
            <h2 id="modal-title" className="text-lg font-semibold text-slate-900">{title}</h2>
            <button onClick={onClose} aria-label="Fechar" className="text-muted-foreground hover:text-foreground transition-colors rounded-full h-8 w-8 flex items-center justify-center hover:bg-secondary"><X className="h-5 w-5" /></button>
          </div>
          <div className="p-6">{children}</div>
        </div>
      </div>
    </div>
  );
}
