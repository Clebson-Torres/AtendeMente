import { expect, test } from "@playwright/test";
import { authenticatedE2EEnabled, loginAsBusinessUser } from "./helpers/auth";

test.describe("authenticated session", () => {
  test.skip(!authenticatedE2EEnabled, "Set E2E_EMAIL and E2E_PASSWORD to run authenticated e2e flows.");

  test("logs in and out through the shell", async ({ page }) => {
    await loginAsBusinessUser(page);
    await expect(page.getByRole("link", { name: "Pacientes" })).toBeVisible();

    await page.getByRole("button", { name: "Sair" }).click();
    await expect(page).toHaveURL(/\/login/);
    await expect(page.getByRole("button", { name: "Entrar" })).toBeVisible();
  });
});
