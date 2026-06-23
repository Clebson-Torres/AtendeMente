import { getCurrentToken } from "./auth";
import { API } from "./api-base";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

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

  let json: ApiResponse<T>;
  try {
    json = await res.json();
  } catch {
    const text = await res.text();
    throw new Error(text || `Erro ${res.status}`);
  }

  if (!res.ok || !json.success) {
    throw new Error(json.message || "Erro na requisição");
  }
  return json.data;
}

export const api = {
  health: () => request<{ status: string; version: string }>("/health"),

  patients: {
    list: (search?: string, page = 1, perPage = 50, status?: string) =>
      request<PaginatedResult<PatientListItem>>(`/patients?search=${search ?? ""}&page=${page}&per_page=${perPage}${status ? `&status=${status}` : ""}`),
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
    appointments: (id: string) =>
      request<CalendarEvent[]>(`/patients/${id}/appointments`),
    importPreview: (contentBase64: string) =>
      request<{ session_id: string; preview: { total_rows: number; valid_rows: number; error_rows: number; rows: any[] } }>("/patients/import/preview", {
        method: "POST",
        body: JSON.stringify({ content_base64: contentBase64 }),
      }),
    importCommit: (sessionId: string, rows: any[]) =>
      request<{ imported: number }>("/patients/import/commit", {
        method: "POST",
        body: JSON.stringify({ session_id: sessionId, rows }),
      }),
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
    update: (id: string, data: Partial<CreateAppointmentInput>) =>
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
    list: (month?: number, year?: number) => {
      let query = "/payments";
      const params: string[] = [];
      if (month !== undefined) params.push(`month=${month}`);
      if (year !== undefined) params.push(`year=${year}`);
      if (params.length) query += "?" + params.join("&");
      return request<PaymentWithAppointment[]>(query);
    },
    pending: () => request<PaymentWithAppointment[]>("/payments/pending"),
    summary: (month?: number, year?: number) => {
      let query = "/payments/summary";
      const params: string[] = [];
      if (month !== undefined) params.push(`month=${month}`);
      if (year !== undefined) params.push(`year=${year}`);
      if (params.length) query += "?" + params.join("&");
      return request<FinancialSummary>(query);
    },
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
    confirm: async (fileId: string): Promise<RecordFile> => {
      if (isTauri()) {
        return tauriInvoke<RecordFile>("cmd_confirm_file_upload", {
          token: getCurrentToken(),
          fileId,
        });
      }
      return request<RecordFile>("/files/confirm", {
        method: "POST",
        body: JSON.stringify({ file_id: fileId }),
      });
    },
    list: async (appointmentId: string): Promise<RecordFile[]> => {
      if (isTauri()) {
        return tauriInvoke<RecordFile[]>("cmd_list_files_by_appointment", {
          token: getCurrentToken(),
          appointmentId,
        });
      }
      return request<RecordFile[]>(`/files/appointment/${appointmentId}`);
    },
    uploadContent: async (fileId: string, data: Blob): Promise<void> => {
      if (isTauri()) {
        const buffer = await data.arrayBuffer();
        const bytes = new Uint8Array(buffer);
        let binary = "";
        for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
        const contentBase64 = btoa(binary);
        return tauriInvoke<void>("cmd_upload_file_content", {
          token: getCurrentToken(),
          fileId,
          contentBase64,
        });
      }
      const res = await fetch(`${API}/files/upload/${fileId}`, {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${getCurrentToken()}`,
          "Content-Type": "application/octet-stream",
        },
        body: data,
      });
      const json = await res.json();
      if (!res.ok || !json.success) throw new Error(json.message || "Upload failed");
    },
    download: async (fileId: string): Promise<{ blob: Blob; fileName: string }> => {
      if (isTauri()) {
        const result = await tauriInvoke<DownloadResult>("cmd_download_file", {
          token: getCurrentToken(),
          fileId,
        });
        const byteString = atob(result.content_base64);
        const bytes = new Uint8Array(byteString.length);
        for (let i = 0; i < byteString.length; i++) bytes[i] = byteString.charCodeAt(i);
        return {
          blob: new Blob([bytes], { type: result.mime_type }),
          fileName: result.original_name,
        };
      }
      const res = await fetch(`${API}/files/${fileId}/download`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      });
      if (!res.ok) {
        const json = await res.json().catch(() => ({}));
        throw new Error(json.message || "Download failed");
      }
      const blob = await res.blob();
      const disposition = res.headers.get("content-disposition");
      const fileName = disposition?.match(/filename="?(.+?)"?$/)?.[1] || "download";
      return { blob, fileName };
    },
    delete: async (fileId: string): Promise<void> => {
      await request(`/files/${fileId}`, { method: "DELETE" });
    },
  },

  exports: {
    patient: async (id: string): Promise<Blob | void> => {
      if (isTauri()) {
        await tauriInvoke<string>("cmd_export_patient_zip", {
          token: getCurrentToken(),
          patientId: id,
        });
        return;
      }
      const res = await fetch(`${API}/exports/patient/${id}`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      });
      if (!res.ok) {
        const json = await res.json().catch(() => ({}));
        throw new Error(json.message || "Export failed");
      }
      return res.blob();
    },
    patientsCsv: async (): Promise<Blob> => {
      const res = await fetch(`${API}/exports/patients/csv`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      });
      if (!res.ok) throw new Error("Erro ao exportar CSV");
      return res.blob();
    },
    appointmentsCsv: async (month?: number, year?: number): Promise<Blob> => {
      let query = "/exports/appointments/csv";
      const params: string[] = [];
      if (month !== undefined) params.push(`month=${month}`);
      if (year !== undefined) params.push(`year=${year}`);
      if (params.length) query += "?" + params.join("&");
      const res = await fetch(`${API}${query}`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      });
      if (!res.ok) throw new Error("Erro ao exportar CSV");
      return res.blob();
    },
    paymentsCsv: async (month?: number, year?: number): Promise<Blob> => {
      let query = "/exports/payments/csv";
      const params: string[] = [];
      if (month !== undefined) params.push(`month=${month}`);
      if (year !== undefined) params.push(`year=${year}`);
      if (params.length) query += "?" + params.join("&");
      const res = await fetch(`${API}${query}`, {
        headers: { Authorization: `Bearer ${getCurrentToken()}` },
      });
      if (!res.ok) throw new Error("Erro ao exportar CSV");
      return res.blob();
    },
  },

  dashboard: () =>
    request<DashboardData>("/dashboard"),

  backup: {
    create: async (password?: string): Promise<{ blob: Blob; fileName: string }> => {
      const token = getCurrentToken();
      const res = await fetch(`${API}/backup`, {
        method: "POST",
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
        body: JSON.stringify({ password: password || null }),
      });
      if (!res.ok) {
        const json = await res.json().catch(() => ({}));
        throw new Error(json.message || "Erro ao criar backup");
      }
      const disposition = res.headers.get("content-disposition");
      const ext = password ? "atendemente" : "zip";
      const fileName = disposition?.match(/filename="?(.+?)"?$/)?.[1] || `backup_${Date.now()}.${ext}`;
      const blob = await res.blob();
      return { blob, fileName };
    },
    restore: (backupBase64: string, password?: string) =>
      request<{ version: number; entries: number }>("/backup/restore", {
        method: "POST",
        body: JSON.stringify({ backup_base64: backupBase64, password: password || null }),
      }),
    getConfig: () =>
      request<BackupConfigData>("/backup/config"),
    setConfig: (frequency: string) =>
      request<void>("/backup/config", {
        method: "PUT",
        body: JSON.stringify({ frequency }),
      }),
  },

  settings: {
    getMobileAccess: () =>
      request<{ enabled: boolean }>("/settings/mobile-access"),
    setMobileAccess: (enabled: boolean) =>
      request<{ enabled: boolean }>("/settings/mobile-access", {
        method: "PUT",
        body: JSON.stringify({ enabled }),
      }),
  },
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

export interface DownloadResult {
  id: string;
  original_name: string;
  mime_type: string;
  byte_size: number;
  uploaded_at: string;
  content_base64: string;
}

export interface FinancialSummary {
  paid_cents: number;
  pending_cents: number;
}

export interface BackupConfigData {
  frequency: string;
  last_backup_at: string | null;
}

export interface DashboardData {
  appointments_count: number;
  todays_appointments: CalendarEvent[];
  upcoming_appointments: CalendarEvent[];
  monthly_appointments: { month: string; count: number }[];
  monthly_financial: { month: string; total_cents: number }[];
}

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number;
  per_page: number;
}
