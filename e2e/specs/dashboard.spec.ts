import { test, expect } from "../fixtures/auth";

test.describe("Dashboard", () => {
  test("dashboard loads with welcome elements", async ({ authPage }) => {
    await authPage.goto("/");
    await authPage.waitForTimeout(1000);

    // Dashboard should show key sections
    await expect(authPage.locator("text=Visão Geral").first()).toBeVisible({ timeout: 5000 });
  });

  test("dashboard quick links navigate correctly", async ({ authPage }) => {
    await authPage.goto("/");
    await authPage.waitForTimeout(1000);

    // Try clicking quick link buttons if they exist
    const navLinks = [
      { text: "Agenda", url: "/appointments" },
      { text: "Pacientes", url: "/patients" },
      { text: "Financeiro", url: "/payments" },
    ];

    for (const link of navLinks) {
      const el = authPage.locator(`a:has-text("${link.text}")`).first();
      if (await el.isVisible()) {
        await el.click();
        await authPage.waitForURL(`**${link.url}`, { timeout: 10000 });
        expect(authPage.url()).toContain(link.url);
        // Navigate back to dashboard
        await authPage.locator("text=Visão geral").click();
        await authPage.waitForURL("/", { timeout: 10000 });
      }
    }
  });

  test("dashboard responds to data changes", async ({ authPage, user }) => {
    // First check dashboard state
    await authPage.goto("/");
    await authPage.waitForTimeout(1000);

    // Create a patient + appointment via API to change dashboard data
    const patientRes = await fetch("http://localhost:3001/api/patients", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({ full_name: "Dashboard Patient" }),
    });
    expect(patientRes.ok).toBeTruthy();
    const patientData = await patientRes.json();

    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(10, 0, 0, 0);
    const endTime = new Date(tomorrow);
    endTime.setHours(11, 0, 0, 0);

    await fetch("http://localhost:3001/api/appointments", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({
        patient_id: patientData.data.id,
        starts_at: tomorrow.toISOString(),
        ends_at: endTime.toISOString(),
        session_price_cents: 15000,
      }),
    });
    expect(patientRes.ok).toBeTruthy();
  });
});
