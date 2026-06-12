import { getCurrentToken } from "./auth";

const API = "http://localhost:3001/api";

type ApiResponse<T> = {
  success: boolean;
  message: string;
  data: T;
};

async function request<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = getCurrentToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${API}${path}`, { ...options, headers });
  const json: ApiResponse<T> = await res.json();

  if (!res.ok || !json.success) {
    throw new Error(json.message || "Erro na requisição");
  }
  return json.data;
}

export const api = {
  health: () => request<{ status: string; version: string }>("/health"),

  patients: {
    list: (search?: string) =>
      request<PatientListItem[]>(`/patients${search ? `?search=${search}` : ""}`),
    get: (id: string) => request<Patient>(`/patients/${id}`),
    create: (data: CreatePatientInput) =>
      request<Patient>("/patients", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    update: (id: string, data: UpdatePatientInput) =>
      request<Patient>(`/patients/${id}`, {
        method: "PUT",
        body: JSON.stringify(data),
      }),
    activate: (id: string) =>
      request<Patient>(`/patients/${id}/activate`, { method: "POST" }),
    deactivate: (id: string) =>
      request<Patient>(`/patients/${id}/deactivate`, { method: "POST" }),
  },

  appointments: {
    calendar: (start: string, end: string) =>
      request<CalendarEvent[]>(`/appointments?start=${start}&end=${end}`),
    get: (id: string) => request<AppointmentDetail>(`/appointments/${id}`),
    create: (data: CreateAppointmentInput) =>
      request<AppointmentDetail>("/appointments", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    update: (id: string, data: CreateAppointmentInput) =>
      request<AppointmentDetail>(`/appointments/${id}`, {
        method: "PUT",
        body: JSON.stringify(data),
      }),
    cancel: (id: string, reason?: string) =>
      request<AppointmentDetail>(`/appointments/${id}/cancel`, {
        method: "POST",
        body: JSON.stringify({ cancel_reason: reason }),
      }),
    cancelSeries: (id: string) =>
      request<void>(`/appointments/series/${id}/cancel`, { method: "POST" }),
  },

  payments: {
    upsert: (data: UpsertPaymentInput) =>
      request<Payment>("/payments/upsert", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    list: () => request<PaymentWithAppointment[]>("/payments"),
    pending: () => request<PaymentWithAppointment[]>("/payments/pending"),
    summary: () =>
      request<FinancialSummary>("/payments/summary"),
  },

  records: {
    save: (data: SaveRecordInput) =>
      request<void>("/records/save", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    get: (appointmentId: string) =>
      request<string>(`/records/${appointmentId}`),
  },

  files: {
    uploadSession: (data: FileUploadRequest) =>
      request<{ file_id: string; storage_path: string }>("/files/upload-session", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    confirm: (fileId: string) =>
      request<RecordFile>("/files/confirm", {
        method: "POST",
        body: JSON.stringify({ file_id: fileId }),
      }),
    download: (id: string) => `${API}/files/${id}/download`,
  },

  exports: {
    patient: (id: string) =>
      fetch(`${API}/exports/patient/${id}`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      }).then((r) => r.blob()),
  },

  dashboard: () =>
    request<DashboardData>("/dashboard"),
};

// ─── Types ───────────────────────────────────────────────────────────────

export interface PatientListItem {
  id: string;
  full_name: string;
  chart_number: string | null;
  phone: string | null;
  email: string | null;
  birth_date: string | null;
  status: string;
  created_at: string;
}

export interface Patient {
  id: string;
  user_id: string;
  full_name: string;
  chart_number: string | null;
  phone: string | null;
  email: string | null;
  birth_date: string | null;
  status: string;
  health_history: string | null;
  medications_in_use: string | null;
  emergency_phone: string | null;
  admin_notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreatePatientInput {
  full_name: string;
  chart_number?: string | null;
  phone?: string | null;
  email?: string | null;
  birth_date?: string | null;
  health_history?: string | null;
  medications_in_use?: string | null;
  emergency_phone?: string | null;
  admin_notes?: string | null;
}

export interface UpdatePatientInput extends CreatePatientInput {}

export interface CalendarEvent {
  id: string;
  patient_id: string;
  title: string;
  start: string;
  end: string;
  status: string;
  confirmation_status: string;
}

export interface AppointmentDetail {
  id: string;
  patient_id: string;
  patient_name: string;
  starts_at: string;
  ends_at: string;
  series_id: string | null;
  status: string;
  confirmation_status: string;
  session_price_cents: number;
  quick_notes: string | null;
  cancel_reason: string | null;
  payment_id: string | null;
  payment_status: string | null;
  payment_method: string | null;
  amount_received_cents: number | null;
  paid_at: string | null;
  payment_notes: string | null;
  record_id: string | null;
}

export interface CreateAppointmentInput {
  patient_id: string;
  starts_at: string;
  ends_at: string;
  status?: string;
  confirmation_status?: string;
  session_price_cents?: number;
  quick_notes?: string;
  cancel_reason?: string;
  recurrence_frequency?: string;
  recurrence_end_mode?: string;
  recurrence_until_date?: string;
  recurrence_occurrences?: number;
}

export interface Payment {
  id: string;
  user_id: string;
  appointment_id: string;
  status: string;
  method: string;
  paid_at: string | null;
  amount_received_cents: number;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface PaymentWithAppointment {
  payment_id: string | null;
  appointment_id: string;
  appointment_status: string;
  status: string;
  method: string;
  amount_received_cents: number;
  paid_at: string | null;
  patient_name: string;
  starts_at: string;
  session_price_cents: number;
}

export interface UpsertPaymentInput {
  appointment_id: string;
  status: string;
  method: string;
  paid_at: string | null;
  amount_received_cents: number;
  notes?: string;
}

export interface SaveRecordInput {
  appointment_id: string;
  patient_id: string;
  content: string;
}

export interface RecordFile {
  id: string;
  user_id: string;
  patient_id: string;
  appointment_id: string;
  payment_id: string | null;
  kind: string;
  storage_path: string;
  original_name: string;
  mime_type: string;
  byte_size: number;
  uploaded_at: string;
}

export interface FileUploadRequest {
  appointment_id: string;
  patient_id: string;
  payment_id?: string;
  kind: "session_attachment" | "payment_receipt";
  file_name: string;
  file_size: number;
  mime_type: string;
}

export interface FinancialSummary {
  paid_cents: number;
  pending_cents: number;
}

export interface DashboardData {
  appointments_count: number;
  todays_appointments: CalendarEvent[];
  upcoming_appointments: CalendarEvent[];
}
