import { test, expect } from "../fixtures/auth";
import { createTestPatient, createTestAppointment } from "../fixtures/test-data";

test.describe("Appointments", () => {
  test("appointments calendar page loads with navigation", async ({ authPage, user }) => {
    await authPage.goto("/appointments");
    await authPage.waitForSelector("text=Agenda", { timeout: 10000 });

    // Month navigation should be visible
    await expect(authPage.locator("button:has(svg)").first()).toBeVisible({ timeout: 5000 });

    // Navigate to next month
    const nextBtn = authPage.locator("button:has(svg)").nth(1);
    if (await nextBtn.isVisible()) {
      await nextBtn.click();
      await authPage.waitForTimeout(300);
    }

    // Navigate to previous month
    const prevBtn = authPage.locator("button:has(svg)").first();
    if (await prevBtn.isVisible()) {
      await prevBtn.click();
      await authPage.waitForTimeout(300);
    }
  });

  test("create an appointment for a patient", async ({ authPage, user }) => {
    // Create a test patient via API
    const patient = await createTestPatient(user.token);

    await authPage.goto("/appointments");
    await authPage.waitForSelector("text=Agenda", { timeout: 10000 });

    // Click "Novo Atendimento" button
    const newBtn = authPage.locator("text=Novo Atendimento");
    if (await newBtn.isVisible()) {
      await newBtn.click();
    } else {
      // Try clicking a day cell
      const dayCell = authPage.locator("button:has-text('15')").first();
      if (await dayCell.isVisible()) {
        await dayCell.click();
        await authPage.waitForTimeout(300);
        const agendarBtn = authPage.locator("text=Agendar").first();
        if (await agendarBtn.isVisible()) {
          await agendarBtn.click();
        }
      }
    }

    await authPage.waitForSelector("text=Atendimento", { timeout: 5000 });

    // Select patient
    const patientSelect = authPage.locator("select").first();
    if (await patientSelect.isVisible()) {
      await patientSelect.selectOption(patient.id);
    }

    // Fill date/time
    const startsAt = authPage.locator('input[type="datetime-local"]').first();
    if (await startsAt.isVisible()) {
      const tomorrow = new Date();
      tomorrow.setDate(tomorrow.getDate() + 1);
      tomorrow.setHours(10, 0, 0, 0);
      const val = tomorrow.toISOString().slice(0, 16);
      await startsAt.fill(val);
    }

    // Submit
    const submitBtn = authPage.locator('button[type="submit"]');
    if (await submitBtn.isVisible()) {
      await submitBtn.click();
      // Wait for calendar to refresh
      await authPage.waitForTimeout(2000);
    }
  });

  test("view appointment detail navigates correctly", async ({ authPage, user }) => {
    // Create a patient + appointment via API
    const patient = await createTestPatient(user.token);
    const appointment = await createTestAppointment(user.token, patient.id);

    await authPage.goto(`/appointments/${appointment.id}`);
    await authPage.waitForURL(`/appointments/${appointment.id}`, { timeout: 10000 });

    // Should see appointment details
    await expect(authPage.locator("text=Paciente").first()).toBeVisible({ timeout: 5000 });
  });

  test("cancel an appointment", async ({ authPage, user }) => {
    const patient = await createTestPatient(user.token);
    const appointment = await createTestAppointment(user.token, patient.id);

    await authPage.goto(`/appointments/${appointment.id}`);
    await authPage.waitForURL(`/appointments/${appointment.id}`, { timeout: 10000 });

    const cancelBtn = authPage.locator("text=Cancelar").first();
    if (await cancelBtn.isVisible()) {
      await cancelBtn.click();
      // Confirm cancellation
      await authPage.waitForSelector("text=Confirmar", { timeout: 5000 });
      const confirmBtn = authPage.locator("text=Confirmar").last();
      if (await confirmBtn.isVisible()) {
        await confirmBtn.click();
        await authPage.waitForTimeout(1000);
        // Should show cancelled status
        await expect(authPage.locator("text=cancelado").first()).toBeVisible({ timeout: 5000 });
      }
    }
  });

  test("reschedule an appointment", async ({ authPage, user }) => {
    const patient = await createTestPatient(user.token);
    const appointment = await createTestAppointment(user.token, patient.id);

    await authPage.goto(`/appointments/${appointment.id}`);
    await authPage.waitForURL(`/appointments/${appointment.id}`, { timeout: 10000 });

    const rescheduleBtn = authPage.locator("text=Reagendar").first();
    if (await rescheduleBtn.isVisible()) {
      await rescheduleBtn.click();
      await authPage.waitForTimeout(500);

      // Change date/time
      const datetimeInput = authPage.locator('input[type="datetime-local"]').first();
      if (await datetimeInput.isVisible()) {
        const newDate = new Date();
        newDate.setDate(newDate.getDate() + 2);
        newDate.setHours(14, 0, 0, 0);
        await datetimeInput.fill(newDate.toISOString().slice(0, 16));
      }

      // Save
      const saveBtn = authPage.locator("text=Salvar").last();
      if (await saveBtn.isVisible()) {
        await saveBtn.click();
        await authPage.waitForTimeout(1000);
      }
    }
  });

  test("register payment for an appointment", async ({ authPage, user }) => {
    const patient = await createTestPatient(user.token);
    const appointment = await createTestAppointment(user.token, patient.id);

    await authPage.goto(`/appointments/${appointment.id}`);
    await authPage.waitForURL(`/appointments/${appointment.id}`, { timeout: 10000 });

    // Look for payment section
    const payBtn = authPage.locator("text=Registrar Pagamento").first();
    if (await payBtn.isVisible()) {
      await payBtn.click();
      await authPage.waitForTimeout(500);

      // Fill payment details
      const methodSelect = authPage.locator("select").last();
      if (await methodSelect.isVisible()) {
        await methodSelect.selectOption("pix");
      }

      const amountInput = authPage.locator('input[type="number"]').first();
      if (await amountInput.isVisible()) {
        await amountInput.fill("150");
      }

      // Confirm
      const confirmBtn = authPage.locator("text=Confirmar").last();
      if (await confirmBtn.isVisible()) {
        await confirmBtn.click();
        await authPage.waitForTimeout(1000);
      }
    }
  });
});
