import { useEffect, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { api, type CalendarEvent, type CreateAppointmentInput, type PatientListItem } from "../lib/api";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import Select from "../components/ui/Select";
import Modal from "../components/ui/Modal";
import StatusBadge from "../components/ui/StatusBadge";
import FieldError from "../components/ui/FieldError";
import { toast } from "../components/ui/Toast";
import { formatTime } from "../lib/format";
import { appointmentSchema, type AppointmentInput } from "../lib/schemas";
import { CalendarDays, ChevronLeft, ChevronRight, Plus, Repeat } from "lucide-react";

const DAY_NAMES = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
const MONTH_NAMES = ["Janeiro","Fevereiro","Março","Abril","Maio","Junho","Julho","Agosto","Setembro","Outubro","Novembro","Dezembro"];

export default function Appointments() {
  const navigate = useNavigate();
  const today = useMemo(() => new Date(), []);
  const [year, setYear] = useState(today.getFullYear());
  const [month, setMonth] = useState(today.getMonth());
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [loading, setLoading] = useState(true);

  const [modalOpen, setModalOpen] = useState(false);
  const [patients, setPatients] = useState<PatientListItem[]>([]);

  const { register, handleSubmit, reset, setValue, watch, formState: { errors } } = useForm<AppointmentInput>({
    resolver: zodResolver(appointmentSchema),
    defaultValues: { patient_id: "", starts_at: "", ends_at: "" },
  });

  const [saving, setSaving] = useState(false);
  const [recurrenceEnabled, setRecurrenceEnabled] = useState(false);
  const recurrenceEndMode = watch("recurrence_end_mode");

  const [dayEvents, setDayEvents] = useState<CalendarEvent[]>([]);
  const [dayPopupOpen, setDayPopupOpen] = useState(false);

  async function loadEvents() {
    setLoading(true);
    try {
      const start = `${year}-${String(month + 1).padStart(2, "0")}-01`;
      const endDate = new Date(year, month + 1, 0);
      const end = `${year}-${String(month + 1).padStart(2, "0")}-${String(endDate.getDate()).padStart(2, "0")}`;
      const data = await api.appointments.calendar(start, end);
      setEvents(data);
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { loadEvents(); }, [year, month]);

  async function openCreate(date?: Date) {
    try {
      const result = await api.patients.list();
      setPatients(result.items.filter((p) => p.status === "active"));
    } catch { setPatients([]); }

    const d = date || new Date();
    d.setMinutes(0, 0, 0);
    reset({
      patient_id: "",
      starts_at: d.toISOString().slice(0, 16),
      ends_at: new Date(d.getTime() + 60 * 60 * 1000).toISOString().slice(0, 16),
      session_price_cents: 0,
    });
    setRecurrenceEnabled(false);
    setModalOpen(true);
  }

  async function onSave(data: AppointmentInput) {
    setSaving(true);
    try {
      const input: CreateAppointmentInput = {
        ...data,
        recurrence_occurrences: data.recurrence_occurrences ? parseInt(data.recurrence_occurrences, 10) : undefined,
      };
      if (!recurrenceEnabled) {
        delete input.recurrence_frequency;
        delete input.recurrence_end_mode;
        delete input.recurrence_occurrences;
        delete input.recurrence_until_date;
      }
      await api.appointments.create(input);
      toast("Atendimento agendado.");
      setModalOpen(false);
      loadEvents();
    } catch (e: any) {
      toast(e.message, "error");
    } finally {
      setSaving(false);
    }
  }

  function clickDay(day: number) {
    const dateStr = `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    const dayEvts = events.filter((e) => e.start.startsWith(dateStr)).sort((a, b) => a.start.localeCompare(b.start));
    if (dayEvts.length === 0) {
      openCreate(new Date(year, month, day));
    } else {
      setDayEvents(dayEvts);
      setDayPopupOpen(true);
    }
  }

  const firstDay = new Date(year, month, 1).getDay();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const prevMonthDays = new Date(year, month, 0).getDate();

  const calendarDays: (number | null)[] = [];
  for (let i = firstDay - 1; i >= 0; i--) calendarDays.push(-(prevMonthDays - i));
  for (let d = 1; d <= daysInMonth; d++) calendarDays.push(d);
  while (calendarDays.length % 7 !== 0) calendarDays.push(null);

  function eventsOnDay(day: number): number {
    const dateStr = `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    return events.filter((e) => e.start.startsWith(dateStr)).length;
  }

  return (
    <div className="p-4 sm:p-6 space-y-5">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <CalendarDays className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-display font-semibold text-slate-900">Agenda</h1>
        </div>
        <Button onClick={() => openCreate()}><Plus className="h-4 w-4 mr-2" />Novo Atendimento</Button>
      </div>

      <div className="app-surface">
        <div className="flex items-center justify-between px-4 py-3">
          <button onClick={() => { if (month === 0) { setYear(y => y - 1); setMonth(11); } else setMonth(m => m - 1); }} className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors text-sm">
            <ChevronLeft className="h-4 w-4" /> Anterior
          </button>
          <span className="font-display text-lg font-semibold text-slate-900">{MONTH_NAMES[month]} {year}</span>
          <button onClick={() => { if (month === 11) { setYear(y => y + 1); setMonth(0); } else setMonth(m => m + 1); }} className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors text-sm">
            Próximo <ChevronRight className="h-4 w-4" />
          </button>
        </div>

        {loading ? (
          <div className="text-center py-12 text-muted-foreground">Carregando...</div>
        ) : (
          <>
            <div className="grid grid-cols-7 border-t border-border">
              {DAY_NAMES.map((d) => <div key={d} className="text-center text-xs font-medium text-muted-foreground py-2 border-b border-border">{d}</div>)}
            </div>
            <div className="grid grid-cols-7">
              {calendarDays.map((day, i) => {
                const isCurrentDay = day === today.getDate() && month === today.getMonth() && year === today.getFullYear();
                const isOtherMonth = day !== null && day < 0;
                const displayDay = day !== null ? (day < 0 ? -day : day) : null;
                const count = day !== null && day > 0 ? eventsOnDay(day) : 0;
  function statusBgColor(status: string): string {
    switch (status) {
      case "completed": return "bg-green-500/80";
      case "cancelled": return "bg-red-500/80";
      case "no_show": return "bg-gray-400/80";
      default: return "bg-primary/80";
    }
  }

  return (
                  <div key={i} onClick={() => day !== null && day > 0 && clickDay(day)}
                    className={`min-h-[90px] border-b border-r border-border/50 p-1.5 cursor-pointer transition-colors hover:bg-accent/50 ${isOtherMonth ? "text-muted-foreground/40" : ""} ${isCurrentDay ? "bg-accent/30" : ""}`}>
                    {displayDay && (
                      <>
                        <div className={`text-xs mb-1 ${isCurrentDay ? "font-bold text-primary" : "text-muted-foreground"}`}>{displayDay}</div>
                        {count > 0 && (
                          <div className="flex flex-wrap gap-0.5">
                            {events.filter(e => e.start.startsWith(`${year}-${String(month + 1).padStart(2, "0")}-${String(displayDay).padStart(2, "0")}`)).slice(0, 3).map(e => (
                              <div key={e.id} className={`w-full text-[10px] truncate text-primary-foreground ${statusBgColor(e.status)} rounded-md px-1 py-0.5 font-medium`}>{e.title}</div>
                            ))}
                            {count > 3 && <div className="text-[10px] text-muted-foreground">+{count - 3} mais</div>}
                          </div>
                        )}
                      </>
                    )}
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>

      <Modal open={dayPopupOpen} onClose={() => setDayPopupOpen(false)} title="Atendimentos do Dia">
        {dayEvents.length === 0 ? (
          <p className="text-muted-foreground text-sm">Nenhum atendimento neste dia.</p>
        ) : (
          <div className="space-y-2">
            {dayEvents.map((e) => (
              <div key={e.id} className="flex items-center justify-between p-3 bg-muted/50 rounded-xl cursor-pointer hover:bg-accent transition-colors"
                onClick={() => { setDayPopupOpen(false); navigate(`/appointments/${e.id}`); }}>
                <div>
                  <p className="text-sm font-medium text-slate-900">{e.title}</p>
                  <p className="text-xs text-muted-foreground">{formatTime(e.start)} - {formatTime(e.end)}</p>
                </div>
                <div className="flex items-center gap-2">
                  <StatusBadge status={e.status} />
                  <StatusBadge status={e.confirmation_status} />
                </div>
              </div>
            ))}
          </div>
        )}
        <div className="mt-4">
          <Button onClick={() => { setDayPopupOpen(false); openCreate(new Date(year, month, parseInt(dayEvents[0]?.start.slice(8, 10) || String(today.getDate())))); }} className="w-full">+ Agendar</Button>
        </div>
      </Modal>

      <Modal open={modalOpen} onClose={() => setModalOpen(false)} title="Novo Atendimento">
        <form onSubmit={handleSubmit(onSave)} className="space-y-4">
          <div>
            <Select label="Paciente *" {...register("patient_id")} options={[{ value: "", label: "Selecione..." }, ...patients.map((p) => ({ value: p.id, label: p.full_name }))]} />
            <FieldError message={errors.patient_id?.message} />
          </div>
          <div>
            <Input label="Início *" type="datetime-local" {...register("starts_at")} />
            <FieldError message={errors.starts_at?.message} />
          </div>
          <div>
            <Input label="Fim *" type="datetime-local" {...register("ends_at")} />
            <FieldError message={errors.ends_at?.message} />
          </div>
          <Input label="Valor da Sessão (R$)" type="number" step="0.01" onChange={(e) => setValue("session_price_cents", Math.round(parseFloat(e.target.value || "0") * 100))} />

          <div className="flex items-center gap-2 pt-1">
            <input type="checkbox" id="recurrence-toggle" checked={recurrenceEnabled}
              onChange={(e) => { setRecurrenceEnabled(e.target.checked); if (!e.target.checked) { setValue("recurrence_frequency", undefined); setValue("recurrence_end_mode", undefined); } }}
              className="h-4 w-4 rounded border-border text-primary focus:ring-primary" />
            <label htmlFor="recurrence-toggle" className="text-sm font-medium flex items-center gap-1 cursor-pointer">
              <Repeat className="h-4 w-4 text-primary" /> Repetir
            </label>
          </div>

          {recurrenceEnabled && (
            <div className="space-y-4 pl-1 border-l-2 border-primary/30 pl-3">
              <div>
                <Select label="Frequência" {...register("recurrence_frequency")}
                  options={[
                    { value: "weekly", label: "Semanal" },
                    { value: "biweekly", label: "Quinzenal" },
                    { value: "monthly", label: "Mensal" },
                  ]} />
                <FieldError message={errors.recurrence_frequency?.message} />
              </div>
              <div>
                <Select label="Encerrar" {...register("recurrence_end_mode")}
                  options={[
                    { value: "occurrences", label: "Após número de sessões" },
                    { value: "until_date", label: "Em data específica" },
                  ]} />
                <FieldError message={errors.recurrence_end_mode?.message} />
              </div>
              {recurrenceEndMode === "occurrences" && (
                <div>
                  <Input label="Número de sessões" type="number" min={2} max={52} defaultValue={4}
                    {...register("recurrence_occurrences")} />
                  <FieldError message={errors.recurrence_occurrences?.message} />
                </div>
              )}
              {recurrenceEndMode === "until_date" && (
                <div>
                  <Input label="Até a data" type="date" {...register("recurrence_until_date")} />
                  <FieldError message={errors.recurrence_until_date?.message} />
                </div>
              )}
            </div>
          )}

          <div className="flex justify-end gap-3 pt-2">
            <Button variant="outline" type="button" onClick={() => setModalOpen(false)}>Cancelar</Button>
            <Button type="submit" disabled={saving}>{saving ? "Salvando..." : "Agendar"}</Button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
