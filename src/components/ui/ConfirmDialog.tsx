import type { ReactNode } from "react";
import Modal from "./Modal";
import Button from "./Button";

interface Props {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmLabel?: string;
  loading?: boolean;
  destructive?: boolean;
  children?: ReactNode;
}

export default function ConfirmDialog({ open, onClose, onConfirm, title, message, confirmLabel = "Confirmar", loading, destructive = true, children }: Props) {
  return (
    <Modal open={open} onClose={onClose} title={title} size="sm">
      {message && <p className="text-sm text-muted-foreground mb-4">{message}</p>}
      {children}
      <div className="flex justify-end gap-3 mt-4">
        <Button variant="outline" onClick={onClose} disabled={loading}>Cancelar</Button>
        <Button variant={destructive ? "destructive" : "default"} onClick={onConfirm} disabled={loading}>{loading ? "Aguarde..." : confirmLabel}</Button>
      </div>
    </Modal>
  );
}
