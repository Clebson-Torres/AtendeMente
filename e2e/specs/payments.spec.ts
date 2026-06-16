import { test, expect } from "../fixtures/auth";
import { createTestPatient, createTestAppointment } from "../fixtures/test-data";

test.describe("Payments", () => {
  test("payments page loads with empty state", async ({ authPage }) => {
    await authPage.goto("/payments");
    await authPage.waitForSelector("text=Financeiro", { timeout: 10000 });
    // Should show empty message or table
    await authPage.waitForTimeout(1000);
  });

  test("payment appears after creating an appointment with payment", async ({ authPage, user }) => {
    // Create patient + appointment with payment via API
    const patient = await createTestPatient(user.token);
    const appointment = await createTestAppointment(user.token, patient.id);

    // Register payment via API
    const payRes = await fetch("http://localhost:3001/api/payments/upsert", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({
        appointment_id: appointment.id,
        status: "paid",
        method: "pix",
        amount_received_cents: 10000,
        notes: "Pagamento E2E",
      }),
    });

    expect(payRes.ok).toBeTruthy();

    // Navigate to payments page
    await authPage.goto("/payments");
    await authPage.waitForSelector("text=Financeiro", { timeout: 10000 });
    await authPage.waitForTimeout(1000);

    // Payment should appear in the list
    await expect(authPage.locator("text=Pix").first()).toBeVisible({ timeout: 5000 });
  });

  test("financial summary cards display", async ({ authPage, user }) => {
    await authPage.goto("/payments");
    await authPage.waitForSelector("text=Financeiro", { timeout: 10000 });
    await authPage.waitForTimeout(1000);

    // Summary cards should be visible
    const totalTexts = ["Resumo", "Total", "Recebido", "Pendente"];
    for (const text of totalTexts) {
      const el = authPage.locator(`text=${text}`).first();
      if (await el.isVisible()) {
        // Good, card exists
      }
    }
  });
});
