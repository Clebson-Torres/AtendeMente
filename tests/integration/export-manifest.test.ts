import { describe, expect, it, vi } from "vitest";
import { buildExportManifest } from "@/features/exports/manifest";

describe("export manifest", () => {
  it("preserves patient data and timeline structure", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-09T12:00:00.000Z"));

    const manifest = buildExportManifest(
      {
        id: "patient-1",
        fullName: "Marina Alves",
        chartNumber: "PR-0001",
      },
      [
        {
          appointmentId: "appointment-1",
          startsAt: new Date("2026-03-09T14:00:00.000Z"),
          endsAt: new Date("2026-03-09T15:00:00.000Z"),
          status: "completed",
          sessionPriceCents: 18000,
          quickNotes: "Sessao presencial",
          paymentStatus: "paid",
          paymentMethod: "pix",
          amountReceivedCents: 18000,
          paidAt: new Date("2026-03-09T15:10:00.000Z"),
          summary: "Resumo protegido",
          files: [
            {
              id: "file-1",
              paymentId: "payment-1",
              kind: "payment_receipt",
              originalName: "nota.pdf",
              mimeType: "application/pdf",
              byteSize: 1234,
              uploadedAt: new Date("2026-03-09T15:12:00.000Z"),
            },
          ],
        },
      ],
    );

    expect(manifest.patient).toMatchObject({ id: "patient-1", chartNumber: "PR-0001" });
    expect(manifest.appointments).toHaveLength(1);
    expect(manifest.appointments[0]?.payment.status).toBe("paid");
    expect(manifest.appointments[0]?.files[0]?.originalName).toBe("nota.pdf");
    expect(manifest.appointments[0]?.files[0]?.kind).toBe("payment_receipt");
    expect(manifest.exportedAt).toBe("2026-03-09T12:00:00.000Z");

    vi.useRealTimers();
  });
});
