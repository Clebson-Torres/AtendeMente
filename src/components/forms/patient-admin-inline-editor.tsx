"use client";

import { useMemo, useState, useTransition } from "react";
import { Pencil } from "lucide-react";
import { toast } from "sonner";
import { healthHistorySuggestions } from "@/features/patients/constants";
import {
  buildHealthHistoryValue,
  stringToHealthHistoryItems,
} from "@/features/patients/health-history";
import { updatePatientAction } from "@/features/patients/actions";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { formatDateBR, formatPhone, maskDateInputBR, parseDateInputBR } from "@/lib/utils";

type PatientAdminValues = {
  fullName: string;
  phone: string;
  emergencyPhone: string;
  birthDate: string;
  email: string;
  healthHistory: string;
  medicationsInUse: string;
  adminNotes: string;
};

type InlineFieldKey = keyof PatientAdminValues;

type PatientAdminInlineEditorProps = {
  patientId: string;
  values: PatientAdminValues;
};

type FieldConfig = {
  key: InlineFieldKey;
  label: string;
  multiline?: boolean;
  placeholder?: string;
  emptyText: string;
  formatDisplay?: (value: string) => string;
  fullWidth?: boolean;
};

const fieldConfigs: FieldConfig[] = [
  { key: "fullName", label: "Nome completo", emptyText: "Nao informado", fullWidth: true },
  { key: "phone", label: "Telefone", placeholder: "(11) 99999-9999", emptyText: "Nao informado", formatDisplay: formatPhone },
  {
    key: "emergencyPhone",
    label: "Telefone de emergencia",
    placeholder: "(11) 98888-7777",
    emptyText: "Nao informado",
    formatDisplay: formatPhone,
  },
  {
    key: "birthDate",
    label: "Nascimento",
    placeholder: "dd/mm/aaaa",
    emptyText: "Nao informado",
    formatDisplay: formatDateBR,
  },
  { key: "email", label: "Email", placeholder: "paciente@email.com", emptyText: "Nao informado", fullWidth: true },
  { key: "healthHistory", label: "Historico de saude", multiline: true, emptyText: "Sem historico de saude registrado.", fullWidth: true },
  { key: "medicationsInUse", label: "Medicamentos em uso", multiline: true, emptyText: "Nenhum medicamento informado.", fullWidth: true },
  { key: "adminNotes", label: "Observacoes", multiline: true, emptyText: "Sem observacoes administrativas registradas.", fullWidth: true },
];

export function PatientAdminInlineEditor({ patientId, values }: PatientAdminInlineEditorProps) {
  const [isPending, startTransition] = useTransition();
  const [draft, setDraft] = useState(values);
  const [editingField, setEditingField] = useState<InlineFieldKey | null>(null);
  const [fieldValue, setFieldValue] = useState("");
  const [healthSearch, setHealthSearch] = useState("");
  const [selectedHealthItems, setSelectedHealthItems] = useState<string[]>([]);

  const filteredSuggestions = useMemo(() => {
    if (editingField !== "healthHistory") {
      return [];
    }

    const query = healthSearch.trim().toLowerCase();

    return healthHistorySuggestions.filter((suggestion) => {
      if (selectedHealthItems.includes(suggestion)) {
        return false;
      }

      if (!query) {
        return false;
      }

      return suggestion.toLowerCase().includes(query);
    });
  }, [editingField, healthSearch, selectedHealthItems]);

  const startEditing = (field: InlineFieldKey) => {
    setEditingField(field);

    if (field === "healthHistory") {
      setSelectedHealthItems(stringToHealthHistoryItems(draft.healthHistory ?? ""));
      setHealthSearch("");
      setFieldValue("");
      return;
    }

    setFieldValue(field === "birthDate" ? formatDateBR(draft[field]) : draft[field] ?? "");
  };

  const cancelEditing = () => {
    setEditingField(null);
    setFieldValue("");
    setHealthSearch("");
    setSelectedHealthItems([]);
  };

  const addHealthItem = (value: string) => {
    const nextValue = value.trim();

    if (!nextValue || selectedHealthItems.includes(nextValue)) {
      setHealthSearch("");
      return;
    }

    const nextItems = [...selectedHealthItems, nextValue];
    setSelectedHealthItems(nextItems);
    setHealthSearch("");
  };

  const removeHealthItem = (value: string) => {
    setSelectedHealthItems((current) => current.filter((item) => item !== value));
  };

  const saveField = (field: InlineFieldKey) => {
    const nextValue =
      field === "birthDate"
        ? parseDateInputBR(fieldValue)
        : field === "healthHistory"
          ? buildHealthHistoryValue(selectedHealthItems, healthSearch)
          : fieldValue.trim();

    const nextDraft = {
      ...draft,
      [field]: nextValue,
    };

    startTransition(async () => {
      const result = await updatePatientAction(patientId, nextDraft);

      if (!result.success) {
        toast.error(result.message);
        return;
      }

      setDraft(nextDraft);
      cancelEditing();
      toast.success("Campo atualizado com sucesso.");
    });
  };

  return (
    <div className="grid gap-4 md:grid-cols-2">
      {fieldConfigs.map((field) => {
        const isEditing = editingField === field.key;
        const rawValue = draft[field.key] ?? "";
        const displayValue = rawValue ? (field.formatDisplay ? field.formatDisplay(rawValue) : rawValue) : field.emptyText;

        return (
          <div
            key={field.key}
            className={`rounded-2xl border border-border/70 bg-white/70 px-4 py-3 ${field.fullWidth ? "md:col-span-2" : ""}`}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <p className="text-xs uppercase tracking-[0.16em] text-muted-foreground">{field.label}</p>

                {isEditing ? (
                  <div className="mt-2 space-y-3">
                    {field.key === "healthHistory" ? (
                      <div className="rounded-2xl border border-input bg-white p-3">
                        <div className="mb-3 flex flex-wrap gap-2">
                          {selectedHealthItems.length ? (
                            selectedHealthItems.map((item) => (
                              <button
                                key={item}
                                className="rounded-full border border-border/80 bg-muted/35 px-3 py-1 text-xs font-medium text-slate-700 transition hover:border-destructive/30 hover:text-destructive"
                                type="button"
                                onClick={() => removeHealthItem(item)}
                              >
                                {item} x
                              </button>
                            ))
                          ) : (
                            <p className="text-sm text-muted-foreground">Adicione condicoes sugeridas ou digite livremente.</p>
                          )}
                        </div>

                        <Input
                          placeholder="Digite para buscar uma sugestao ou adicione um item livre"
                          value={healthSearch}
                          onChange={(event) => setHealthSearch(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === "Enter" || event.key === ",") {
                              event.preventDefault();
                              addHealthItem(healthSearch.replace(/,$/, ""));
                            }
                          }}
                        />

                        {filteredSuggestions.length ? (
                          <div className="mt-3 flex flex-wrap gap-2">
                            {filteredSuggestions.slice(0, 8).map((suggestion) => (
                              <button
                                key={suggestion}
                                className="rounded-full border border-primary/20 bg-primary/5 px-3 py-1.5 text-xs font-medium text-primary transition hover:bg-primary/10"
                                type="button"
                                onClick={() => addHealthItem(suggestion)}
                              >
                                {suggestion}
                              </button>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    ) : field.multiline ? (
                      <Textarea value={fieldValue} onChange={(event) => setFieldValue(event.target.value)} />
                    ) : (
                      <Input
                        placeholder={field.placeholder}
                        value={fieldValue}
                        onChange={(event) =>
                          setFieldValue(field.key === "birthDate" ? maskDateInputBR(event.target.value) : event.target.value)
                        }
                      />
                    )}

                    <div className="flex flex-wrap gap-2">
                      <Button disabled={isPending} size="sm" type="button" onClick={() => saveField(field.key)}>
                        {isPending ? "Salvando..." : "Salvar"}
                      </Button>
                      <Button disabled={isPending} size="sm" type="button" variant="outline" onClick={cancelEditing}>
                        Cancelar
                      </Button>
                    </div>
                  </div>
                ) : (
                  <p className="mt-2 whitespace-pre-line text-sm leading-6 text-slate-700">{displayValue}</p>
                )}
              </div>

              {!isEditing ? (
                <Button
                  aria-label={`Editar ${field.label}`}
                  className="h-8 w-8 rounded-full px-0"
                  size="sm"
                  type="button"
                  variant="ghost"
                  onClick={() => startEditing(field.key)}
                >
                  <Pencil className="h-4 w-4" />
                </Button>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );
}
