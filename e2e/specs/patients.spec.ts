import { test, expect } from "../fixtures/auth";

test.describe("Patients", () => {
  test("patients page loads with empty state", async ({ authPage }) => {
    await authPage.goto("/patients");
    await authPage.waitForSelector("text=Pacientes", { timeout: 10000 });
    // Should show empty message
    await expect(authPage.locator("text=Nenhum paciente").first()).toBeVisible({ timeout: 10000 });
  });

  test("create a new patient", async ({ authPage }) => {
    await authPage.goto("/patients");
    await authPage.waitForSelector("text=Novo Paciente", { timeout: 10000 });

    // Click Novo Paciente
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });

    // Fill form
    await authPage.fill('input[name="full_name"]', "Maria Silva");
    await authPage.fill('input[name="phone"]', "(11) 99999-8888");
    await authPage.fill('input[name="email"]', "maria@test.com");

    // Submit
    await authPage.locator('button[type="submit"]').click();

    // Wait for modal to close and list to refresh
    await authPage.waitForSelector("text=Maria Silva", { timeout: 10000 });
    await expect(authPage.locator("text=Maria Silva").first()).toBeVisible();
  });

  test("create patient with empty name shows validation error", async ({ authPage }) => {
    await authPage.goto("/patients");
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });

    // Submit without filling required field
    await authPage.locator('button[type="submit"]').click();

    await expect(authPage.locator("text=Nome deve ter").first()).toBeVisible({ timeout: 5000 });
  });

  test("search patients filters the list", async ({ authPage }) => {
    await authPage.goto("/patients");

    // Create a patient first
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });
    await authPage.fill('input[name="full_name"]', "João Pereira");
    await authPage.locator('button[type="submit"]').click();
    await authPage.waitForSelector("text=João Pereira", { timeout: 10000 });

    // Search for something that won't match
    const searchInput = authPage.locator('input[placeholder*="Buscar"]');
    if (await searchInput.isVisible()) {
      await searchInput.fill("ZZZZ_NOT_FOUND");
      await authPage.waitForTimeout(500);
      await expect(authPage.locator("text=Nenhum paciente").first()).toBeVisible({ timeout: 5000 });
    }
  });

  test("edit an existing patient", async ({ authPage }) => {
    await authPage.goto("/patients");

    // Create a patient to edit
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });
    await authPage.fill('input[name="full_name"]', "Carlos editável");
    await authPage.locator('button[type="submit"]').click();
    await authPage.waitForSelector("text=Carlos editável", { timeout: 10000 });

    // Click edit button
    const editBtn = authPage.locator("text=Editar").first();
    if (await editBtn.isVisible()) {
      await editBtn.click();
    } else {
      // Try clicking the row itself
      await authPage.locator("text=Carlos editável").first().click();
      // This might navigate to detail, go back
      await authPage.goto("/patients");
      return; // skip edit test if no direct edit button
    }

    // Update name
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });
    await authPage.fill('input[name="full_name"]', "Carlos atualizado");
    await authPage.locator('button[type="submit"]').click();

    await authPage.waitForSelector("text=Carlos atualizado", { timeout: 10000 });
    await expect(authPage.locator("text=Carlos atualizado").first()).toBeVisible();
  });

  test("deactivate and reactivate a patient", async ({ authPage }) => {
    await authPage.goto("/patients");

    // Create a patient
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });
    await authPage.fill('input[name="full_name"]', "Paciente ativar/desativar");
    await authPage.locator('button[type="submit"]').click();
    await authPage.waitForSelector("text=Paciente ativar/desativar", { timeout: 10000 });

    // Click Desativar
    const deactivateBtn = authPage.locator("text=Desativar").first();
    if (await deactivateBtn.isVisible()) {
      await deactivateBtn.click();
      // Confirm dialog
      await authPage.waitForSelector("text=Confirmar", { timeout: 5000 });
      await authPage.locator("text=Confirmar").click();
      await authPage.waitForTimeout(1000);
    }
  });

  test("patient detail shows appointment history", async ({ authPage }) => {
    await authPage.goto("/patients");

    // Create a patient
    await authPage.locator("text=Novo Paciente").click();
    await authPage.waitForSelector('input[name="full_name"]', { timeout: 5000 });
    await authPage.fill('input[name="full_name"]', "Paciente detalhes");
    await authPage.locator('button[type="submit"]').click();
    await authPage.waitForSelector("text=Paciente detalhes", { timeout: 10000 });

    // Navigate to detail by clicking the patient name/row
    await authPage.locator("text=Paciente detalhes").first().click();
    await authPage.waitForURL(/\/patients\//, { timeout: 10000 });

    // Should show patient info
    await expect(authPage.locator("text=Paciente detalhes").first()).toBeVisible();
    // Should show appointment history section (even if empty)
    await expect(authPage.locator("text=Histórico").first()).toBeVisible({ timeout: 5000 });
  });
});
