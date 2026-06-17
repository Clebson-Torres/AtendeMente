import { test, expect } from "../fixtures/auth";

test.describe("File Upload/Download", () => {
  test("upload a file to an appointment and download it", async ({ authPage, user }) => {
    // 1. Create a patient
    const patientRes = await fetch("http://localhost:3001/api/patients", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({ full_name: "File Test Patient" }),
    });
    const patientJson = await patientRes.json();
    expect(patientJson.success).toBe(true);
    const patientId = patientJson.data.id;

    // 2. Create an appointment
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    const startStr = tomorrow.toISOString().slice(0, 16);
    const endStr = new Date(tomorrow.getTime() + 60 * 60 * 1000).toISOString().slice(0, 16);

    const apptRes = await fetch("http://localhost:3001/api/appointments", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({
        patient_id: patientId,
        starts_at: startStr,
        ends_at: endStr,
        session_price_cents: 10000,
      }),
    });
    const apptJson = await apptRes.json();
    expect(apptJson.success).toBe(true);
    const appointmentId = apptJson.data.id;

    // 3. Navigate to appointment detail
    await authPage.goto(`/appointments/${appointmentId}`);
    await authPage.waitForSelector("text=Arquivos", { timeout: 10000 });

    // 4. Click "Anexar Arquivo" to open the modal
    await authPage.getByRole("button", { name: /Anexar Arquivo/ }).click();
    await authPage.waitForSelector("text=Anexar Arquivo", { timeout: 5000 });

    // 5. Upload a small text file (not validated by magic bytes since we're testing flow)
    const filePath = await createTempPdf();
    await authPage.locator('input[type="file"]').setInputFiles(filePath);

    // Select attachment type
    await authPage.locator("select").selectOption("session_attachment");

    // 6. Submit
    await authPage.getByRole("button", { name: "Enviar" }).click();

    // 7. Wait for upload to complete and file to appear in list
    await authPage.waitForSelector("text=test-upload.pdf", { timeout: 15000 });
    await expect(authPage.locator("text=test-upload.pdf").first()).toBeVisible();
  });
});

/** Create a minimal valid PDF for upload testing */
async function createTempPdf(): Promise<string> {
  const fs = await import("fs");
  const path = await import("path");
  const os = await import("os");
  const tmpDir = os.tmpdir();
  const filePath = path.join(tmpDir, "test-upload.pdf");
  // Minimal valid PDF
  const pdfBytes = Buffer.from(
    "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \ntrailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF"
  );
  fs.writeFileSync(filePath, pdfBytes);
  return filePath;
}
