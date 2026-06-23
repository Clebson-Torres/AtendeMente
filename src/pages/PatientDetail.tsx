import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { api, type Patient, type CalendarEvent } from "../lib/api";
import Button from "../components/ui/Button";
import StatusBadge from "../components/ui/StatusBadge";
import { formatDate, formatTime } from "../lib/format";
import { downloadFile } from "../lib/utils";
import { DetailSkeleton } from "../components/ui/Skeleton";
import { ArrowLeft, User, Phone, Calendar, FileText, Download, MessageCircle, Video } from "lucide-react";

export default function PatientDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [patient, setPatient] = useState<Patient | null>(null);
  const [appointments, setAppointments] = useState<CalendarEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    Promise.all([
      api.patients.get(id),
      api.patients.appointments(id),
    ])
      .then(([p, a]) => {
        setPatient(p);
        setAppointments(a);
      })
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [id]);

  async function handleExport() {
    if (!id) return;
    setExporting(true);
    try {
      const blob = await api.exports.patient(id);
      if (blob) {
        await downloadFile(blob, `paciente-${patient?.full_name.replace(/\s+/g, "_")}.zip`);
      }
    } catch (e: any) {
      setError(e.message || "Erro ao exportar");
    } finally {
      setExporting(false);
    }
  }

  if (loading) return <div className="p-6"><DetailSkeleton /></div>;
  if (error) return <div className="p-6 text-destructive">{error}</div>;
  if (!patient) return <div className="p-6 text-muted-foreground">Paciente não encontrado.</div>;

  return (
    <div className="p-4 sm:p-6 space-y-6 max-w-4xl">
      <button onClick={() => navigate("/patients")} className="flex items-center gap-1 text-sm text-primary hover:text-primary/80 transition-colors">
        <ArrowLeft className="h-4 w-4" /> Voltar para Pacientes
      </button>

      <div className="app-surface p-6">
          <div className="flex items-start justify-between flex-wrap gap-4">
            <div className="flex items-center gap-4">
              <div className="h-14 w-14 rounded-full bg-accent flex items-center justify-center">
                <User className="h-6 w-6 text-accent-foreground" />
              </div>
              <div>
                <h1 className="text-2xl font-display font-semibold text-slate-900">{patient.full_name}</h1>
                <div className="flex items-center gap-2 mt-1">
                  <StatusBadge status={patient.status} />
                  {patient.chart_number && (
                    <span className="text-xs text-muted-foreground">Prontuário: {patient.chart_number}</span>
                  )}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button size="sm" onClick={() => navigate(`/appointments?patientId=${id}`)}>
                <Calendar className="h-4 w-4 mr-2" />Agendar
              </Button>
              <a href="https://meet.google.com/new" target="_blank" rel="noopener noreferrer">
                <Button size="sm" variant="outline">
                  <Video className="h-4 w-4 mr-2" />Iniciar Atendimento
                </Button>
              </a>
              <Button variant="outline" size="sm" onClick={handleExport} disabled={exporting}>
                <Download className="h-4 w-4 mr-2" />{exporting ? "Exportando..." : "Exportar ZIP"}
              </Button>
            </div>
          </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mt-6">
          {patient.phone && (
            <div className="flex items-center gap-2 text-sm">
              <Phone className="h-4 w-4 text-muted-foreground" />
              <span>{patient.phone}</span>
              <a
                href={`https://wa.me/55${patient.phone.replace(/\D/g, "")}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-green-600 hover:text-green-700 transition-colors"
                title="Abrir WhatsApp"
              >
                <MessageCircle className="h-4 w-4" />
              </a>
            </div>
          )}
          {patient.email && (
            <div className="flex items-center gap-2 text-sm">
              <span className="text-muted-foreground">Email:</span>
              <span>{patient.email}</span>
            </div>
          )}
          {patient.birth_date && (
            <div className="flex items-center gap-2 text-sm">
              <Calendar className="h-4 w-4 text-muted-foreground" />
              <span>{formatDate(patient.birth_date)}</span>
            </div>
          )}
        </div>

        {patient.emergency_phone && (
          <p className="text-sm text-muted-foreground mt-2">Emergência: {patient.emergency_phone}</p>
        )}

        {patient.admin_notes && (
          <div className="mt-4 p-3 bg-muted/50 rounded-xl">
            <p className="text-xs text-muted-foreground font-medium mb-1">Observações Administrativas</p>
            <p className="text-sm">{patient.admin_notes}</p>
          </div>
        )}
      </div>

      <div className="app-surface p-6">
        <div className="flex items-center gap-2 mb-4">
          <FileText className="h-5 w-5 text-primary" />
          <h2 className="text-lg font-display font-semibold text-slate-900">Histórico de Atendimentos</h2>
        </div>

        {appointments.length === 0 ? (
          <p className="text-muted-foreground text-sm">Nenhum atendimento registrado.</p>
        ) : (
          <div className="space-y-2">
            {appointments.map((a) => (
              <div
                key={a.id}
                onClick={() => navigate(`/appointments/${a.id}`)}
                className="flex items-center justify-between p-3 bg-muted/50 rounded-xl cursor-pointer hover:bg-accent transition-colors"
              >
                <div>
                  <p className="text-sm font-medium text-slate-900">{formatDate(a.start)}</p>
                  <p className="text-xs text-muted-foreground">{formatTime(a.start)} - {formatTime(a.end)}</p>
                </div>
                <div className="flex items-center gap-2">
                  <StatusBadge status={a.status} />
                  <StatusBadge status={a.confirmation_status} />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {patient.health_history && (
        <div className="app-surface p-6">
          <h3 className="font-semibold text-slate-900 mb-2">Histórico de Saúde</h3>
          <p className="text-sm text-muted-foreground whitespace-pre-wrap">{patient.health_history}</p>
        </div>
      )}

      {patient.medications_in_use && (
        <div className="app-surface p-6">
          <h3 className="font-semibold text-slate-900 mb-2">Medicações em Uso</h3>
          <p className="text-sm text-muted-foreground whitespace-pre-wrap">{patient.medications_in_use}</p>
        </div>
      )}
    </div>
  );
}
