import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { api, type Patient, type CalendarEvent } from "../lib/api";
import Button from "../components/ui/Button";
import StatusBadge from "../components/ui/StatusBadge";
import { formatDate, formatTime } from "../lib/format";
import { downloadFile, mensagemDeSalvamento } from "../lib/utils";
import { toast } from "../components/ui/Toast";
import { DetailSkeleton } from "../components/ui/Skeleton";
import { ArrowLeft, User, Phone, Calendar, FileText, Download, Video } from "lucide-react";

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
    const ctrl = new AbortController();
    setLoading(true);
    Promise.all([
      api.patients.get(id),
      api.patients.appointments(id),
    ])
      .then(([p, a]) => {
        if (!ctrl.signal.aborted) {
          setPatient(p);
          setAppointments(a);
        }
      })
      .catch((e) => { if (!ctrl.signal.aborted) setError(e.message); })
      .finally(() => { if (!ctrl.signal.aborted) setLoading(false); });
    return () => ctrl.abort();
  }, [id]);

  async function handleExport() {
    if (!id) return;
    setExporting(true);
    try {
      const blob = await api.exports.patient(id);
      if (blob) {
        const r = await downloadFile(blob, `paciente-${patient?.full_name.replace(/\s+/g, "_")}.zip`);
        const msg = mensagemDeSalvamento(r, "Dossiê do paciente");
        if (msg) toast(msg);
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
                href={`https://wa.me/${patient.phone.replace(/\D/g, "").length > 10 ? patient.phone.replace(/\D/g, "") : `55${patient.phone.replace(/\D/g, "")}`}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-green-600 hover:text-green-700 transition-colors"
                title="Abrir WhatsApp"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor"><path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z"/></svg>
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
