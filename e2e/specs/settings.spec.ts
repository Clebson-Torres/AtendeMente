import { test, expect } from "../fixtures/auth";

test.describe("Settings", () => {
  test("page loads with all configuration sections", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Configurações", { timeout: 10000 });

    await expect(authPage.locator("text=Backup Manual").first()).toBeVisible({ timeout: 5000 });
    await expect(authPage.locator("text=Restaurar Backup").first()).toBeVisible({ timeout: 5000 });
    await expect(authPage.locator("text=Backup Automático").first()).toBeVisible({ timeout: 5000 });
  });

  test("export backup opens password modal", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Backup Manual", { timeout: 10000 });

    const exportBtn = authPage.getByRole("button", { name: /Exportar/i });
    await expect(exportBtn).toBeVisible({ timeout: 5000 });
    await exportBtn.click();

    await expect(authPage.locator("text=Proteger Backup com Senha").first()).toBeVisible({ timeout: 5000 });
    await expect(authPage.locator('input[placeholder*="Senha"]').first()).toBeVisible();
  });

  test("export backup shows error for short password", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Backup Manual", { timeout: 10000 });

    const exportBtn = authPage.getByRole("button", { name: /Exportar/i });
    await exportBtn.click();

    await authPage.waitForSelector('input[placeholder*="Senha"]', { timeout: 5000 });
    const pwInput = authPage.locator('input[placeholder*="Senha"]').first();
    await pwInput.fill("short");
    await authPage.waitForTimeout(300);

    const confirmBtn = authPage.getByRole("button", { name: /Exportar/i }).last();
    await expect(confirmBtn).toBeDisabled();
  });

  test("export backup shows error for mismatched passwords", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Backup Manual", { timeout: 10000 });

    const exportBtn = authPage.getByRole("button", { name: /Exportar/i });
    await exportBtn.click();

    await authPage.waitForSelector('input[placeholder*="Senha"]', { timeout: 5000 });
    const pwInputs = authPage.locator('input[type="password"]');
    await pwInputs.nth(0).fill("test12345678");
    await pwInputs.nth(1).fill("different1234");
    await authPage.waitForTimeout(300);

    const confirmBtn = authPage.getByRole("button", { name: /Exportar/i }).last();
    await expect(confirmBtn).toBeDisabled();
  });

  test("toggle auto-backup on and save", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Backup Automático", { timeout: 10000 });

    const toggle = authPage.locator('input[role="switch"]').first();
    await toggle.check({ force: true });
    await authPage.waitForTimeout(500);

    const saveBtn = authPage.getByRole("button", { name: /Salvar/i });
    await expect(saveBtn).toBeVisible({ timeout: 5000 });
    await saveBtn.click();

    await expect(authPage.locator("text=Configuração salva").first()).toBeVisible({ timeout: 5000 });
  });

  test("cancel export backup modal", async ({ authPage }) => {
    await authPage.goto("/settings");
    await authPage.waitForSelector("text=Backup Manual", { timeout: 10000 });

    const exportBtn = authPage.getByRole("button", { name: /Exportar/i });
    await exportBtn.click();

    await authPage.waitForSelector("text=Proteger Backup com Senha", { timeout: 5000 });

    const cancelBtn = authPage.getByRole("button", { name: /Cancelar/i });
    await cancelBtn.click();

    await expect(authPage.locator("text=Proteger Backup com Senha")).not.toBeVisible({ timeout: 3000 });
  });
});
