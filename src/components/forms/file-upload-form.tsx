"use client";

import { useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { createBrowserSupabaseClient } from "@/lib/supabase/browser";
import { type RecordFileKind, recordFileKindLabelMap } from "@/types/domain";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type FileUploadFormProps = {
  appointmentId: string;
  patientId: string;
  paymentId?: string;
  kind?: RecordFileKind;
  title?: string;
  buttonLabel?: string;
  onUploaded?: () => void;
};

export function FileUploadForm({
  appointmentId,
  patientId,
  paymentId,
  kind = "session_attachment",
  title,
  buttonLabel,
  onUploaded,
}: FileUploadFormProps) {
  const router = useRouter();
  const [file, setFile] = useState<File | null>(null);
  const [isPending, startTransition] = useTransition();
  const inputId = `file-upload-${kind}-${paymentId ?? appointmentId}`;

  const handleUpload = () => {
    if (!file) {
      toast.error("Selecione um arquivo antes de continuar.");
      return;
    }

    startTransition(async () => {
      const prepareResponse = await fetch("/api/files/upload", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          appointmentId,
          patientId,
          paymentId,
          kind,
          fileName: file.name,
          fileSize: file.size,
          mimeType: file.type,
        }),
      });

      const preparePayload = await prepareResponse.json();

      if (!prepareResponse.ok) {
        toast.error(preparePayload.message ?? "Nao foi possivel iniciar o upload.");
        return;
      }

      const supabase = createBrowserSupabaseClient();
      const fileBuffer = await file.arrayBuffer();
      const uploadResult = await supabase.storage
        .from(preparePayload.bucket)
        .uploadToSignedUrl(preparePayload.path, preparePayload.token, fileBuffer, {
          contentType: file.type,
        });

      if (uploadResult.error) {
        toast.error("Falha ao enviar o arquivo com seguranca.");
        return;
      }

      const confirmResponse = await fetch("/api/files/upload", {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          fileId: preparePayload.fileId,
        }),
      });

      if (!confirmResponse.ok) {
        toast.error("Upload concluido, mas a confirmacao falhou.");
        return;
      }

      toast.success("Arquivo anexado com seguranca.");
      setFile(null);
      router.refresh();
      onUploaded?.();
    });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor={inputId}>{title ?? recordFileKindLabelMap[kind]}</Label>
        <Input
          id={inputId}
          accept=".pdf,.doc,.docx,.png,.jpg,.jpeg"
          onChange={(event) => setFile(event.target.files?.[0] ?? null)}
          type="file"
        />
      </div>

      <Button disabled={isPending} onClick={handleUpload} type="button">
        {isPending ? "Enviando..." : buttonLabel ?? "Enviar anexo"}
      </Button>
    </div>
  );
}
