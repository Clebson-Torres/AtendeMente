import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { api } from "../src/lib/api";

// Mock modules
vi.mock("../src/lib/api", () => ({
  api: {
    appointments: {
      get: vi.fn(),
      update: vi.fn(),
    },
    patients: {
      list: vi.fn(),
    },
    payments: {
      upsert: vi.fn(),
    },
    records: {
      get: vi.fn(),
      save: vi.fn(),
    },
    files: {
      list: vi.fn(),
    },
  },
}));

vi.mock("../src/components/ui/Toast", () => ({
  toast: vi.fn(),
}));

import AppointmentDetail from "../src/pages/AppointmentDetail";

const mockAppointment = {
  id: "appt-1",
  patient_id: "pat-1",
  patient_name: "João Silva",
  starts_at: "2026-06-15T09:00:00",
  ends_at: "2026-06-15T10:00:00",
  series_id: null,
  status: "scheduled",
  confirmation_status: "confirmed",
  session_price_cents: 15000,
  quick_notes: null,
  cancel_reason: null,
  payment_id: null,
  payment_status: null,
  payment_method: null,
  amount_received_cents: null,
  paid_at: null,
  payment_notes: null,
  record_id: null,
  encrypted_payload: null,
  iv: null,
  auth_tag: null,
  key_version: null,
};

function renderComponent() {
  return render(
    <MemoryRouter initialEntries={["/appointments/appt-1"]}>
      <Routes>
        <Route path="/appointments/:id" element={<AppointmentDetail />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("AppointmentDetail - Complete / No-show", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (api.appointments.get as any).mockResolvedValue(mockAppointment);
    (api.files.list as any).mockResolvedValue([]);
  });

  it("renders complete and no-show buttons when status is scheduled", async () => {
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("Concluir Atendimento")).toBeInTheDocument();
      expect(screen.getByText("Não Compareceu")).toBeInTheDocument();
    });
  });

  it("calls update with completed status when clicking Concluir", async () => {
    (api.appointments.update as any).mockResolvedValue({ ...mockAppointment, status: "completed" });
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("Concluir Atendimento")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Concluir Atendimento"));

    await waitFor(() => {
      expect(api.appointments.update).toHaveBeenCalledWith("appt-1", { status: "completed" });
    });
  });

  it("calls update with no_show status when clicking Não Compareceu", async () => {
    (api.appointments.update as any).mockResolvedValue({ ...mockAppointment, status: "no_show" });
    renderComponent();

    await waitFor(() => {
      expect(screen.getByText("Não Compareceu")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Não Compareceu"));

    await waitFor(() => {
      expect(api.appointments.update).toHaveBeenCalledWith("appt-1", { status: "no_show" });
    });
  });
});
