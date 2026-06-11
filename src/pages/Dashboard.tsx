import { useEffect, useState } from "react";
import { api, type CalendarEvent } from "../lib/api";
import { useNavigate } from "react-router-dom";
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

  if (error) return <div className="p-6 text-red-500">{error}</div>;
  if (!data) return <div className="p-6 text-gray-500">Carregando...</div>;

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-2xl font-bold text-gray-800">Dashboard</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-white rounded-xl shadow-sm p-4 border border-gray-100">
          <p className="text-sm text-gray-500">Atendimentos no mês</p>
          <p className="text-3xl font-bold text-blue-600">{data.appointments_count}</p>
        </div>
        <div className="bg-white rounded-xl shadow-sm p-4 border border-gray-100">
          <p className="text-sm text-gray-500">A receber</p>
          <p className="text-3xl font-bold text-yellow-600">{formatBRL(finSummary.pending_cents)}</p>
        </div>
        <div className="bg-white rounded-xl shadow-sm p-4 border border-gray-100">
          <p className="text-sm text-gray-500">Recebido</p>
          <p className="text-3xl font-bold text-green-600">{formatBRL(finSummary.paid_cents)}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="bg-white rounded-xl shadow-sm p-4 border border-gray-100">
          <h2 className="font-semibold text-gray-700 mb-3">Atendimentos de hoje</h2>
          {data.todays_appointments.length === 0 ? (
            <p className="text-gray-400 text-sm">Nenhum atendimento hoje</p>
          ) : (
            <ul className="space-y-2">
              {data.todays_appointments.map((a) => (
                <li
                  key={a.id}
                  onClick={() => navigate(`/appointments/${a.id}`)}
                  className="flex justify-between items-center p-2 bg-gray-50 rounded cursor-pointer hover:bg-blue-50"
                >
                  <span className="text-sm font-medium">{a.title}</span>
                  <span className="text-xs text-gray-500">{a.start.slice(11, 16)} - {a.end.slice(11, 16)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="bg-white rounded-xl shadow-sm p-4 border border-gray-100">
          <h2 className="font-semibold text-gray-700 mb-3">Próximos atendimentos</h2>
          {data.upcoming_appointments.length === 0 ? (
            <p className="text-gray-400 text-sm">Nenhum atendimento agendado</p>
          ) : (
            <ul className="space-y-2">
              {data.upcoming_appointments.map((a) => (
                <li
                  key={a.id}
                  onClick={() => navigate(`/appointments/${a.id}`)}
                  className="flex justify-between items-center p-2 bg-gray-50 rounded cursor-pointer hover:bg-blue-50"
                >
                  <div>
                    <p className="text-sm font-medium">{a.title}</p>
                    <p className="text-xs text-gray-400">{a.start.slice(8, 10)}/{a.start.slice(5, 7)}</p>
                  </div>
                  <span className="text-xs text-gray-500">{a.start.slice(11, 16)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
