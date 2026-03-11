import { describe, expect, it } from "vitest";
import {
  fileUploadRequestSchema,
  getFileExtension,
  isAllowedMimeTypeForFile,
} from "@/features/files/schemas";

describe("file upload validation", () => {
  it("detects supported receipt extensions", () => {
    expect(getFileExtension("recibo.pdf")).toBe(".pdf");
    expect(getFileExtension("recibo.PNG")).toBe(".png");
    expect(getFileExtension("recibo.jpeg")).toBe(".jpeg");
  });

  it("accepts matching pdf, png and jpeg mime types", () => {
    expect(isAllowedMimeTypeForFile("recibo.pdf", "application/pdf")).toBe(true);
    expect(isAllowedMimeTypeForFile("recibo.png", "image/png")).toBe(true);
    expect(isAllowedMimeTypeForFile("recibo.jpg", "image/jpeg")).toBe(true);
    expect(isAllowedMimeTypeForFile("recibo.jpeg", "image/jpg")).toBe(true);
  });

  it("rejects mismatched mime type for the informed extension", () => {
    expect(isAllowedMimeTypeForFile("recibo.pdf", "image/png")).toBe(false);
    expect(isAllowedMimeTypeForFile("recibo.png", "application/pdf")).toBe(false);
  });

  it("rejects schema payload when mime type does not match the extension", () => {
    const result = fileUploadRequestSchema.safeParse({
      appointmentId: "3e0ff82e-5e6b-4a4b-80a3-b36e60993f81",
      patientId: "4bcefeeb-84d3-40ab-b97f-30d1aa18da80",
      paymentId: "7334bb4f-0c57-4675-a3d9-2ea8f84b4aeb",
      kind: "payment_receipt",
      fileName: "recibo.pdf",
      fileSize: 2048,
      mimeType: "image/png",
    });

    expect(result.success).toBe(false);
    expect(result.error?.flatten().fieldErrors.mimeType?.[0]).toContain("nao corresponde");
  });
});
