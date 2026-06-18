import { useEffect, useState } from "react";
import { api, type DashboardData } from "../lib/api";
import { useNavigate } from "react-router-dom";
import { CalendarDays, UsersRound, TrendingUp } from "lucide-react";
import { formatBRL } from "../lib/format";
import { CardSkeleton, DetailSkeleton } from "../components/ui/Skeleton";
import Skeleton from "../components/ui/Skeleton";
import {
  BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, LineChart, Line,
} from "recharts";

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

  if (!data) return (
    <div className="p-4 sm:p-6 space-y-6">
      <Skeleton className="h-8 w-48" />
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <CardSkeleton /><CardSkeleton /><CardSkeleton />
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="app-surface p-5"><DetailSkeleton /></div>
        <div className="app-surface p-5"><DetailSkeleton /></div>
      </div>
    </div>
  );

  const apptData = data.monthly_appointments.map((m) => ({
    month: m.month.slice(5),
    atendimentos: m.count,
  }));

  const finData = data.monthly_financial.map((m) => ({
    month: m.month.slice(5),
    receita: m.total_cents / 100,
  }));

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

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="app-surface p-5">
          <h2 className="font-semibold text-slate-900 mb-3">Atendimentos por mês</h2>
          {apptData.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
              <CalendarDays className="h-8 w-8 mb-2 opacity-40" />
              <p className="text-sm">Nenhum atendimento nos últimos 12 meses</p>
            </div>
          ) : (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={apptData}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="month" tick={{ fontSize: 12 }} stroke="hsl(var(--muted-foreground))" />
                <YAxis tick={{ fontSize: 12 }} stroke="hsl(var(--muted-foreground))" />
                <Tooltip />
                <Bar dataKey="atendimentos" fill="hsl(var(--primary))" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>

        <div className="app-surface p-5">
          <h2 className="font-semibold text-slate-900 mb-3">Receita mensal</h2>
          {finData.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
              <TrendingUp className="h-8 w-8 mb-2 opacity-40" />
              <p className="text-sm">Nenhuma receita nos últimos 12 meses</p>
            </div>
          ) : (
            <ResponsiveContainer width="100%" height={220}>
              <LineChart data={finData}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis dataKey="month" tick={{ fontSize: 12 }} stroke="hsl(var(--muted-foreground))" />
                <YAxis tick={{ fontSize: 12 }} stroke="hsl(var(--muted-foreground))" tickFormatter={(v) => `R$${v}`} />
                <Tooltip formatter={(v) => [`R$ ${Number(v).toFixed(2)}`, "Receita"]} />
                <Line type="monotone" dataKey="receita" stroke="hsl(var(--primary))" strokeWidth={2} dot={{ fill: "hsl(var(--primary))" }} />
              </LineChart>
            </ResponsiveContainer>
          )}
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