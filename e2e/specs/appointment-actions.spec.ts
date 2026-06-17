import { test, expect } from "../fixtures/auth";

test.describe("Appointment Actions", () => {
  test("Concluir Atendimento changes status to completed", async ({ authPage, user }) => {
    // Create patient
    const patientRes = await fetch("http://localhost:3001/api/patients", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({ full_name: "Action Test Patient" }),
    });
    const patientJson = await patientRes.json();
    const patientId = patientJson.data.id;

    // Create appointment
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
    const appointmentId = apptJson.data.id;

    // Navigate to appointment detail
    await authPage.goto(`/appointments/${appointmentId}`);
    await authPage.waitForSelector("text=Concluir Atendimento", { timeout: 10000 });

    // Click Concluir
    await authPage.getByRole("button", { name: /Concluir Atendimento/ }).click();

    // Wait for status badge to reflect "completed"
    await authPage.waitForSelector("text=completed", { timeout: 10000 });

    // Verify via API
    const getRes = await fetch(`http://localhost:3001/api/appointments/${appointmentId}`, {
      headers: { Authorization: `Bearer ${user.token}` },
    });
    const getJson = await getRes.json();
    expect(getJson.data.status).toBe("completed");
  });

  test("Não Compareceu changes status to no_show", async ({ authPage, user }) => {
    // Create patient
    const patientRes = await fetch("http://localhost:3001/api/patients", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${user.token}`,
      },
      body: JSON.stringify({ full_name: "No-Show Test Patient" }),
    });
    const patientJson = await patientRes.json();
    const patientId = patientJson.data.id;

    // Create appointment
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
      }),
    });
    const apptJson = await apptRes.json();
    const appointmentId = apptJson.data.id;

    // Navigate to appointment detail
    await authPage.goto(`/appointments/${appointmentId}`);
    await authPage.waitForSelector("text=Não Compareceu", { timeout: 10000 });

    // Click Não Compareceu
    await authPage.getByRole("button", { name: /Não Compareceu/ }).click();

    // Wait for status badge to reflect "no_show"
    await authPage.waitForSelector("text=no_show", { timeout: 10000 });

    // Verify via API
    const getRes = await fetch(`http://localhost:3001/api/appointments/${appointmentId}`, {
      headers: { Authorization: `Bearer ${user.token}` },
    });
    const getJson = await getRes.json();
    expect(getJson.data.status).toBe("no_show");
  });
});
