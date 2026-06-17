import { test, expect } from "../fixtures/auth";

test.describe("Exports", () => {
  test("export patient ZIP downloads a file", async ({ authPage, user }) => {
    // Create a patient via API
    const res = await fetch("http://localhost:3001/api/patients", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({ full_name: "Export Test Patient" }),
    });
    const json = await res.json();
    expect(json.success).toBe(true);
    const patientId = json.data.id;

    // Navigate to patient detail
    await authPage.goto(`/patients/${patientId}`);
    await authPage.waitForSelector("text=Export Test Patient", { timeout: 10000 });

    // Click export button and wait for download
    const [download] = await Promise.all([
      authPage.waitForEvent("download", { timeout: 15000 }),
      authPage.getByRole("button", { name: /Exportar/ }).click(),
    ]);

    expect(download.suggestedFilename()).toContain(".zip");
    const path = await download.path();
    expect(path).toBeTruthy();
  });
});
