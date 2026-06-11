import { useEffect, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { api, type CalendarEvent, type CreateAppointmentInput, type PatientListItem } from "../lib/api";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import Select from "../components/ui/Select";
import Modal from "../components/ui/Modal";
import StatusBadge from "../components/ui/StatusBadge";
import { toast } from "../components/ui/Toast";
import { formatTime } from "../lib/format";

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
  const [form, setForm] = useState<CreateAppointmentInput>({
    patient_id: "",
    starts_at: "",
    ends_at: "",
  });
  const [saving, setSaving] = useState(false);

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
      const list = await api.patients.list();
      setPatients(list.filter((p) => p.status === "active"));
    } catch { setPatients([]); }

    const d = date || new Date();
    d.setMinutes(0, 0, 0);
    const startStr = d.toISOString().slice(0, 16);
    const endStr = new Date(d.getTime() + 60 * 60 * 1000).toISOString().slice(0, 16);

    setForm({ patient_id: "", starts_at: startStr, ends_at: endStr, status: "scheduled", confirmation_status: "pending", session_price_cents: 0 });
    setModalOpen(true);
  }

  async function handleSave() {
    if (!form.patient_id || !form.starts_at || !form.ends_at) {
      toast("Preencha paciente, início e fim.", "error");
      return;
    }
    setSaving(true);
    try {
      await api.appointments.create(form);
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
    <div className="p-6 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-800">Agenda</h1>
        <Button onClick={() => openCreate()}>+ Novo Atendimento</Button>
      </div>

      {/* Month navigation */}
      <div className="flex items-center justify-between bg-white rounded-xl shadow-sm border border-gray-100 px-4 py-2">
        <button onClick={() => { if (month === 0) { setYear(y => y - 1); setMonth(11); } else setMonth(m => m - 1); }} className="text-gray-500 hover:text-gray-700">&larr; Anterior</button>
        <span className="text-lg font-semibold text-gray-700">{MONTH_NAMES[month]} {year}</span>
        <button onClick={() => { if (month === 11) { setYear(y => y + 1); setMonth(0); } else setMonth(m => m + 1); }} className="text-gray-500 hover:text-gray-700">Próximo &rarr;</button>
      </div>

      {loading ? (
        <div className="text-center py-12 text-gray-400">Carregando...</div>
      ) : (
        <div className="bg-white rounded-xl shadow-sm border border-gray-100">
          {/* Day headers */}
          <div className="grid grid-cols-7 border-b border-gray-200">
            {DAY_NAMES.map((d) => (
              <div key={d} className="text-center text-xs font-medium text-gray-500 py-2">{d}</div>
            ))}
          </div>
          {/* Calendar grid */}
          <div className="grid grid-cols-7">
            {calendarDays.map((day, i) => {
              const isCurrentDay = day === today.getDate() && month === today.getMonth() && year === today.getFullYear();
              const isOtherMonth = day !== null && day < 0;
              const displayDay = day !== null ? (day < 0 ? -day : day) : null;
              const count = day !== null && day > 0 ? eventsOnDay(day) : 0;

              return (
                <div
                  key={i}
                  onClick={() => day !== null && day > 0 && clickDay(day)}
                  className={`min-h-[80px] border-b border-r border-gray-100 p-1 cursor-pointer transition-colors hover:bg-blue-50 ${isOtherMonth ? "text-gray-300" : ""} ${isCurrentDay ? "bg-blue-50" : ""}`}
                >
                  {displayDay && (
                    <>
                      <div className={`text-xs mb-1 ${isCurrentDay ? "font-bold text-blue-600" : "text-gray-500"}`}>{displayDay}</div>
                      {count > 0 && (
                        <div className="flex flex-wrap gap-0.5">
                          {events.filter(e => e.start.startsWith(`${year}-${String(month + 1).padStart(2, "0")}-${String(displayDay).padStart(2, "0")}`)).slice(0, 3).map(e => (
                            <div key={e.id} className="w-full text-[10px] truncate text-blue-700 bg-blue-50 rounded px-0.5">{e.title}</div>
                          ))}
                          {count > 3 && <div className="text-[10px] text-gray-400">+{count - 3} mais</div>}
                        </div>
                      )}
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Day popup */}
      <Modal open={dayPopupOpen} onClose={() => setDayPopupOpen(false)} title="Atendimentos do Dia">
        {dayEvents.length === 0 ? (
          <p className="text-gray-400 text-sm">Nenhum atendimento neste dia.</p>
        ) : (
          <div className="space-y-2">
            {dayEvents.map((e) => (
              <div key={e.id} className="flex items-center justify-between p-3 bg-gray-50 rounded-lg cursor-pointer hover:bg-blue-50" onClick={() => { setDayPopupOpen(false); navigate(`/appointments/${e.id}`); }}>
                <div>
                  <p className="text-sm font-medium text-gray-700">{e.title}</p>
                  <p className="text-xs text-gray-400">{formatTime(e.start)} - {formatTime(e.end)}</p>
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

      {/* Create modal */}
      <Modal open={modalOpen} onClose={() => setModalOpen(false)} title="Novo Atendimento">
        <div className="space-y-4">
          <Select
            label="Paciente *"
            value={form.patient_id}
            onChange={(e) => setForm({ ...form, patient_id: e.target.value })}
            options={[{ value: "", label: "Selecione..." }, ...patients.map((p) => ({ value: p.id, label: p.full_name }))]}
          />
          <Input label="Início *" type="datetime-local" value={form.starts_at} onChange={(e) => setForm({ ...form, starts_at: e.target.value })} />
          <Input label="Fim *" type="datetime-local" value={form.ends_at} onChange={(e) => setForm({ ...form, ends_at: e.target.value })} />
          <Input label="Valor da Sessão (R$)" type="number" step="0.01" value={(form.session_price_cents || 0) / 100} onChange={(e) => setForm({ ...form, session_price_cents: Math.round(parseFloat(e.target.value || "0") * 100) })} />
          <div className="flex justify-end gap-3 pt-2">
            <Button variant="secondary" onClick={() => setModalOpen(false)}>Cancelar</Button>
            <Button onClick={handleSave} disabled={saving}>{saving ? "Salvando..." : "Agendar"}</Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
