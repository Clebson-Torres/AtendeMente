import { describe, it, expect, vi, beforeEach } from "vitest";
import { api } from "../src/lib/api";

vi.mock("../src/lib/auth", () => ({
  getCurrentToken: () => "test-token",
}));

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("CSV exports", () => {
  it("exports patients CSV returns a Blob", async () => {
    const mockBlob = new Blob(["nome,status\nAna,active"], { type: "text/csv" });
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });
    vi.stubGlobal("fetch", mockFetch);

    const blob = await api.exports.patientsCsv();

    expect(blob).toBeInstanceOf(Blob);
    const text = await blob.text();
    expect(text).toContain("Ana");
    expect(mockFetch.mock.calls[0][0]).toContain("/exports/patients/csv");
  });

  it("exports appointments CSV with month/year params", async () => {
    const mockBlob = new Blob(["paciente,data\nJoão,2026-03-01"], { type: "text/csv" });
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.exports.appointmentsCsv(3, 2026);

    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("month=3");
    expect(url).toContain("year=2026");
    expect(url).toContain("/exports/appointments/csv");
  });

  it("exports payments CSV with month/year params", async () => {
    const mockBlob = new Blob(["paciente,valor\nMaria,15000"], { type: "text/csv" });
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.exports.paymentsCsv(3, 2026);

    const url = mockFetch.mock.calls[0][0];
    expect(url).toContain("month=3");
    expect(url).toContain("year=2026");
    expect(url).toContain("/exports/payments/csv");
  });

  it("exports payments CSV without params uses base URL", async () => {
    const mockBlob = new Blob(["paciente,valor\nMaria,15000"], { type: "text/csv" });
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });
    vi.stubGlobal("fetch", mockFetch);

    await api.exports.paymentsCsv();

    const url = mockFetch.mock.calls[0][0];
    expect(url).not.toContain("month=");
    expect(url).not.toContain("year=");
  });

  it("throws error when export fails", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: false,
    });
    vi.stubGlobal("fetch", mockFetch);

    await expect(api.exports.patientsCsv()).rejects.toThrow("Erro ao exportar CSV");
  });
});
