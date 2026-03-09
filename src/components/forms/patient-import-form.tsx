"use client";

import { useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type PreviewPayload = {
  rows: Array<{
    rowNumber: number;
    fullName: string;
    phone: string;
    email: string;
  }>;
  errors: Array<{ row: number; message: string }>;
  duplicates: Array<{ row: number; duplicateOf: number; fullName: string }>;
  existingDuplicates: Array<{ row: number; existingPatientId: string; fullName: string }>;
  summary: {
    totalRows: number;
    validRows: number;
    invalidRows: number;
    duplicateRows: number;
  };
};

async function readJsonResponse(response: Response) {
  const text = await response.text();

  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text) as Record<string, unknown>;
  } catch {
    return null;
  }
}

export function PatientImportForm() {
  const router = useRouter();
  const [file, setFile] = useState<File | null>(null);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [isPending, startTransition] = useTransition();

  const buildFormData = () => {
    const formData = new FormData();
    if (file) {
      formData.append("file", file);
    }
    return formData;
  };

  const handlePreview = () => {
    if (!file) {
      toast.error("Selecione um arquivo CSV antes de continuar.");
      return;
    }

    startTransition(async () => {
      const response = await fetch("/api/patients/import/preview", {
        method: "POST",
        body: buildFormData(),
      });

      const payload = (await readJsonResponse(response)) as PreviewPayload & { message?: string } | null;

      if (!response.ok) {
        toast.error(payload?.message ?? "Nao foi possivel analisar o arquivo.");
        return;
      }

      setPreview(payload as PreviewPayload);
      toast.success("Preview de importacao gerado.");
    });
  };

  const handleImport = () => {
    if (!file || !preview) {
      return;
    }

    startTransition(async () => {
      const response = await fetch("/api/patients/import/commit", {
        method: "POST",
        body: buildFormData(),
      });

      const payload = (await readJsonResponse(response)) as
        | { importedCount?: number; message?: string }
        | null;

      if (!response.ok) {
        toast.error(payload?.message ?? "Nao foi possivel importar os pacientes.");
        return;
      }

      toast.success(`${payload?.importedCount ?? 0} pacientes importados com sucesso.`);
      setFile(null);
      setPreview(null);
      router.refresh();
    });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="patients-import-file">Arquivo CSV</Label>
        <Input
          id="patients-import-file"
          accept=".csv"
          type="file"
          onChange={(event) => {
            setFile(event.target.files?.[0] ?? null);
            setPreview(null);
          }}
        />
      </div>

      <div className="flex flex-wrap gap-3">
        <Button type="button" variant="outline" disabled={isPending} onClick={handlePreview}>
          {isPending ? "Analisando..." : "Gerar preview"}
        </Button>
        <Button
          type="button"
          disabled={isPending || !preview || preview.summary.invalidRows > 0}
          onClick={handleImport}
        >
          {isPending ? "Importando..." : "Importar pacientes"}
        </Button>
      </div>

      <div className="rounded-3xl bg-muted/35 p-4 text-sm text-muted-foreground">
        Use um CSV exportado do Excel com colunas como: nome, telefone, email, nascimento, telefone de emergencia,
        medicamentos, historico de saude e observacoes.
      </div>

      {preview ? (
        <div className="space-y-4 rounded-3xl border border-border/80 bg-white p-4">
          <div className="grid gap-3 sm:grid-cols-4">
            <div className="rounded-2xl bg-muted/35 p-3">
              <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">Linhas</p>
              <p className="mt-1 text-lg font-semibold">{preview.summary.totalRows}</p>
            </div>
            <div className="rounded-2xl bg-muted/35 p-3">
              <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">Validas</p>
              <p className="mt-1 text-lg font-semibold">{preview.summary.validRows}</p>
            </div>
            <div className="rounded-2xl bg-muted/35 p-3">
              <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">Invalidas</p>
              <p className="mt-1 text-lg font-semibold">{preview.summary.invalidRows}</p>
            </div>
            <div className="rounded-2xl bg-muted/35 p-3">
              <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">Duplicadas</p>
              <p className="mt-1 text-lg font-semibold">{preview.summary.duplicateRows}</p>
            </div>
          </div>

          {preview.errors.length ? (
            <div className="rounded-2xl border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive">
              {preview.errors.map((error) => (
                <p key={`${error.row}-${error.message}`}>Linha {error.row}: {error.message}</p>
              ))}
            </div>
          ) : null}

          {preview.duplicates.length ? (
            <div className="rounded-2xl border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900">
              {preview.duplicates.map((duplicate) => (
                <p key={`${duplicate.row}-${duplicate.duplicateOf}`}>
                  Linha {duplicate.row}: {duplicate.fullName} parece duplicar a linha {duplicate.duplicateOf}.
                </p>
              ))}
            </div>
          ) : null}

          {preview.existingDuplicates.length ? (
            <div className="rounded-2xl border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900">
              {preview.existingDuplicates.map((duplicate) => (
                <p key={`${duplicate.row}-${duplicate.existingPatientId}`}>
                  Linha {duplicate.row}: {duplicate.fullName} ja existe na sua base e sera ignorado na importacao.
                </p>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
