import { useEffect, useState } from "react";
import { api, type CalendarEvent } from "../lib/api";
import { useNavigate } from "react-router-dom";
import { CalendarDays, UsersRound, TrendingUp } from "lucide-react";
import { formatBRL } from "../lib/format";

interface DashboardData {
  appointments_count: number;
  todays_appointments: CalendarEvent[];
  upcoming_appointments: CalendarEvent[];
}

export default function Dashboard() {
  const navigate = useNavigate();
  const [data, setData] = useState<DashboardData | null>(null);
  const [finSummary, setFinSummary] = useState({ paid_cents: 0, pending_cents: 0 });
  const [error, setError] = useState("");

  useEffect(() => {
    Promise.all([
      api.dashboard(),
      api.payments.summary(),
    ])
      .then(([d, f]) => {
        setData(d);
        setFinSummary(f);
      })
      .catch((e) => setError(e.message));
  }, []);

  if (error) return <div className="p-6 text-destructive">{error}</div>;
  if (!data) return <div className="p-6 text-center py-12 text-muted-foreground">Carregando...</div>;

  return (
    <div className="p-4 sm:p-6 space-y-6">
      <h1 className="text-2xl font-display font-semibold text-slate-900">Visão geral</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">Atendimentos no mês</p>
          <p className="text-3xl font-bold text-primary mt-1">{data.appointments_count}</p>
        </div>
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">A receber</p>
          <p className="text-3xl font-bold text-yellow-600 mt-1">{formatBRL(finSummary.pending_cents)}</p>
        </div>
        <div className="app-surface p-5">
          <p className="text-sm text-muted-foreground">Recebido</p>
          <p className="text-3xl font-bold text-success mt-1">{formatBRL(finSummary.paid_cents)}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <button onClick={() => navigate("/appointments")} className="app-surface p-5 text-left hover:shadow-md transition-shadow">
          <CalendarDays className="h-6 w-6 text-primary mb-2" />
          <p className="font-medium text-slate-900">Abrir agenda</p>
          <p className="text-sm text-muted-foreground">Ver e gerenciar atendimentos</p>
        </button>
        <button onClick={() => navigate("/patients")} className="app-surface p-5 text-left hover:shadow-md transition-shadow">
          <UsersRound className="h-6 w-6 text-primary mb-2" />
          <p className="font-medium text-slate-900">Ver pacientes</p>
          <p className="text-sm text-muted-foreground">Cadastrar e buscar pacientes</p>
        </button>
        <button onClick={() => navigate("/payments")} className="app-surface p-5 text-left hover:shadow-md transition-shadow">
          <TrendingUp className="h-6 w-6 text-primary mb-2" />
          <p className="font-medium text-slate-900">Financeiro</p>
          <p className="text-sm text-muted-foreground">Controle de pagamentos</p>
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="app-surface p-5">
          <h2 className="font-semibold text-slate-900 mb-3">Atendimentos de hoje</h2>
          {data.todays_appointments.length === 0 ? (
            <p className="text-muted-foreground text-sm">Nenhum atendimento hoje</p>
          ) : (
            <ul className="space-y-2">
              {data.todays_appointments.map((a) => (
                <li
                  key={a.id}
                  onClick={() => navigate(`/appointments/${a.id}`)}
                  className="flex justify-between items-center p-3 bg-muted/50 rounded-xl cursor-pointer hover:bg-accent transition-colors"
                >
                  <span className="text-sm font-medium text-slate-900">{a.title}</span>
                  <span className="text-xs text-muted-foreground">{a.start.slice(11, 16)} - {a.end.slice(11, 16)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="app-surface p-5">
          <h2 className="font-semibold text-slate-900 mb-3">Próximos atendimentos</h2>
          {data.upcoming_appointments.length === 0 ? (
            <p className="text-muted-foreground text-sm">Nenhum atendimento agendado</p>
          ) : (
            <ul className="space-y-2">
              {data.upcoming_appointments.map((a) => (
                <li
                  key={a.id}
                  onClick={() => navigate(`/appointments/${a.id}`)}
                  className="flex justify-between items-center p-3 bg-muted/50 rounded-xl cursor-pointer hover:bg-accent transition-colors"
                >
                  <div>
                    <p className="text-sm font-medium text-slate-900">{a.title}</p>
                    <p className="text-xs text-muted-foreground">{a.start.slice(8, 10)}/{a.start.slice(5, 7)}</p>
                  </div>
                  <span className="text-xs text-muted-foreground">{a.start.slice(11, 16)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
