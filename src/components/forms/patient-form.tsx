"use client";

import { useEffect, useMemo, useState, useTransition } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { toast } from "sonner";
import { createPatientAction, updatePatientAction } from "@/features/patients/actions";
import { healthHistorySuggestions } from "@/features/patients/constants";
import { patientFormSchema, type PatientFormInput } from "@/features/patients/schemas";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { FieldError } from "@/components/forms/field-error";
import { formatDateInputBR, maskDateInputBR, parseDateInputBR } from "@/lib/utils";

type PatientFormProps = {
  patientId?: string;
  defaultValues?: Partial<PatientFormInput>;
  submitLabel?: string;
};

function stringToItems(value: string) {
  return value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

function itemsToString(items: string[]) {
  return items.join("\n");
}

function buildHealthHistoryValue(items: string[], draftValue: string) {
  const nextDraft = draftValue.trim();

  if (!nextDraft || items.includes(nextDraft)) {
    return itemsToString(items);
  }

  return itemsToString([...items, nextDraft]);
}

export function PatientForm({ patientId, defaultValues, submitLabel = "Salvar paciente" }: PatientFormProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();
  const [birthDateInput, setBirthDateInput] = useState(() => formatDateInputBR(defaultValues?.birthDate));
  const [healthSearch, setHealthSearch] = useState("");
  const [selectedHealthItems, setSelectedHealthItems] = useState<string[]>(() =>
    stringToItems(defaultValues?.healthHistory ?? ""),
  );
  const [duplicatePatientId, setDuplicatePatientId] = useState<string | null>(null);

  const form = useForm<PatientFormInput>({
    resolver: zodResolver(patientFormSchema),
    defaultValues: {
      fullName: defaultValues?.fullName ?? "",
      phone: defaultValues?.phone ?? "",
      email: defaultValues?.email ?? "",
      birthDate: defaultValues?.birthDate ?? "",
      healthHistory: defaultValues?.healthHistory ?? "",
      medicationsInUse: defaultValues?.medicationsInUse ?? "",
      emergencyPhone: defaultValues?.emergencyPhone ?? "",
      adminNotes: defaultValues?.adminNotes ?? "",
    },
  });

  useEffect(() => {
    setBirthDateInput(formatDateInputBR(defaultValues?.birthDate));
    form.setValue("birthDate", defaultValues?.birthDate ?? "");
  }, [defaultValues?.birthDate, form]);

  useEffect(() => {
    const nextItems = stringToItems(defaultValues?.healthHistory ?? "");
    setSelectedHealthItems(nextItems);
    form.setValue("healthHistory", itemsToString(nextItems));
  }, [defaultValues?.healthHistory, form]);

  const filteredSuggestions = useMemo(() => {
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
  }, [healthSearch, selectedHealthItems]);

  const addHealthItem = (value: string) => {
    const nextValue = value.trim();

    if (!nextValue || selectedHealthItems.includes(nextValue)) {
      setHealthSearch("");
      return;
    }

    const nextItems = [...selectedHealthItems, nextValue];
    setSelectedHealthItems(nextItems);
    form.setValue("healthHistory", itemsToString(nextItems), { shouldDirty: true, shouldValidate: true });
    setHealthSearch("");
  };

  const removeHealthItem = (value: string) => {
    const nextItems = selectedHealthItems.filter((item) => item !== value);
    setSelectedHealthItems(nextItems);
    form.setValue("healthHistory", itemsToString(nextItems), { shouldDirty: true, shouldValidate: true });
  };

  const onSubmit = form.handleSubmit((values) => {
    startTransition(async () => {
      setDuplicatePatientId(null);
      const payload = {
        ...values,
        healthHistory: buildHealthHistoryValue(selectedHealthItems, healthSearch),
      };
      const result = patientId ? await updatePatientAction(patientId, payload) : await createPatientAction(payload);

      if (!result.success) {
        setDuplicatePatientId(result.data?.duplicatePatientId ?? null);
        toast.error(result.message);
        return;
      }

      toast.success(result.message);

      if (!patientId && result.data?.patientId) {
        router.push(`/patients/${result.data.patientId}`);
      }

      router.refresh();
    });
  });

  return (
    <form className="space-y-5" onSubmit={onSubmit}>
      <input type="hidden" {...form.register("birthDate")} />
      <input type="hidden" {...form.register("healthHistory")} />

      <div className="grid gap-5 md:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="fullName">Nome completo</Label>
          <Input id="fullName" {...form.register("fullName")} />
          <FieldError message={form.formState.errors.fullName?.message} />
        </div>

        <div className="space-y-2">
          <Label htmlFor="birthDateInput">Data de nascimento</Label>
          <Input
            id="birthDateInput"
            inputMode="numeric"
            placeholder="dd/mm/aaaa"
            value={birthDateInput}
            onChange={(event) => {
              const nextValue = maskDateInputBR(event.target.value);
              setBirthDateInput(nextValue);
              form.setValue("birthDate", parseDateInputBR(nextValue), { shouldValidate: true, shouldDirty: true });
            }}
          />
          <FieldError message={form.formState.errors.birthDate?.message} />
        </div>

        <div className="space-y-2">
          <Label htmlFor="phone">Telefone</Label>
          <Input id="phone" placeholder="(11) 99999-9999" {...form.register("phone")} />
          <FieldError message={form.formState.errors.phone?.message} />
        </div>

        <div className="space-y-2">
          <Label htmlFor="emergencyPhone">Telefone de emergencia</Label>
          <Input id="emergencyPhone" placeholder="(11) 98888-7777" {...form.register("emergencyPhone")} />
          <FieldError message={form.formState.errors.emergencyPhone?.message} />
        </div>

        <div className="space-y-2 md:col-span-2">
          <Label htmlFor="email">Email</Label>
          <Input id="email" type="email" placeholder="paciente@email.com" {...form.register("email")} />
          <FieldError message={form.formState.errors.email?.message} />
        </div>
      </div>

      {duplicatePatientId ? (
        <div className="rounded-3xl border border-amber-300 bg-amber-50 p-4 text-sm text-amber-900">
          Ja existe um paciente com este nome e telefone.
          {" "}
          <Link className="font-semibold underline" href={`/patients/${duplicatePatientId}`}>
            Abrir ficha existente
          </Link>
        </div>
      ) : null}

      <div className="space-y-2">
        <Label htmlFor="healthHistorySearch">Historico de saude</Label>
        <div className="rounded-3xl border border-input bg-white p-3">
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
            id="healthHistorySearch"
            placeholder="Digite para buscar uma sugestao ou adicione um item livre"
            value={healthSearch}
            onChange={(event) => {
              const nextValue = event.target.value;
              setHealthSearch(nextValue);
              form.setValue("healthHistory", buildHealthHistoryValue(selectedHealthItems, nextValue), {
                shouldDirty: true,
                shouldValidate: true,
              });
            }}
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
        <FieldError message={form.formState.errors.healthHistory?.message} />
      </div>

      <div className="space-y-2">
        <Label htmlFor="medicationsInUse">Medicamentos em uso</Label>
        <Textarea
          id="medicationsInUse"
          placeholder="Ex.: Sertralina 50mg, uso diario pela manha."
          {...form.register("medicationsInUse")}
        />
        <FieldError message={form.formState.errors.medicationsInUse?.message} />
      </div>

      <div className="space-y-2">
        <Label htmlFor="adminNotes">Observacoes administrativas</Label>
        <Textarea
          id="adminNotes"
          placeholder="Informacoes de contexto, combinados ou observacoes administrativas."
          {...form.register("adminNotes")}
        />
        <FieldError message={form.formState.errors.adminNotes?.message} />
      </div>

      <Button disabled={isPending} type="submit">
        {isPending ? "Salvando..." : submitLabel}
      </Button>
    </form>
  );
}
