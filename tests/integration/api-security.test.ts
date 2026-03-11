import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppError } from "@/lib/errors/app-error";

const getCurrentUser = vi.fn();
const enforceRateLimit = vi.fn();
const exportPatientBundle = vi.fn();

vi.mock("@/lib/auth/session", () => ({
  getCurrentUser,
}));

vi.mock("@/lib/security/rate-limit", () => ({
  enforceRateLimit,
}));

vi.mock("@/features/exports/service", () => ({
  exportPatientBundle,
}));

describe("api security surfaces", () => {
  beforeEach(() => {
    getCurrentUser.mockReset();
    enforceRateLimit.mockReset();
    exportPatientBundle.mockReset();
  });

  it("returns 401 when exporting without session", async () => {
    getCurrentUser.mockResolvedValue(null);
    const { GET } = await import("@/app/api/patients/[patientId]/export/route");

    const response = await GET(new Request("http://localhost/api/patients/patient-1/export"), {
      params: Promise.resolve({ patientId: "patient-1" }),
    });

    expect(response.status).toBe(401);
  });

  it("maps rate-limit failures in export route", async () => {
    getCurrentUser.mockResolvedValue({ id: "user-1" });
    enforceRateLimit.mockRejectedValue(new AppError("Muitas tentativas em pouco tempo.", { statusCode: 429 }));
    const { GET } = await import("@/app/api/patients/[patientId]/export/route");

    const response = await GET(new Request("http://localhost/api/patients/patient-1/export"), {
      params: Promise.resolve({ patientId: "patient-1" }),
    });
    const payload = await response.json();

    expect(response.status).toBe(429);
    expect(payload.message).toContain("Muitas tentativas");
  });

  it("rejects invalid upload extensions before starting upload", async () => {
    getCurrentUser.mockResolvedValue({ id: "user-1" });
    enforceRateLimit.mockResolvedValue(undefined);
    const { POST } = await import("@/app/api/files/upload/route");

    const response = await POST(
      new Request("http://localhost/api/files/upload", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          appointmentId: "3e0ff82e-5e6b-4a4b-80a3-b36e60993f81",
          patientId: "4bcefeeb-84d3-40ab-b97f-30d1aa18da80",
          kind: "session_attachment",
          fileName: "malicioso.exe",
          fileSize: 1024,
          mimeType: "application/pdf",
        }),
      }),
    );
    const payload = await response.json();

    expect(response.status).toBe(400);
    expect(payload.errors?.fileName?.[0]).toContain("Extensao");
  }, 15_000);
});
