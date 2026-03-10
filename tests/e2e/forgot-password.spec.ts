import { expect, test } from "@playwright/test";

test("submits forgot-password with generic confirmation", async ({ page }) => {
  await page.goto("/forgot-password");

  await expect(page.getByRole("heading", { name: "Recuperar acesso" })).toBeVisible();
  await page.getByLabel("Email").fill("qa-forgot-password@atendemente.local");
  await page.getByRole("button", { name: "Enviar link de redefinicao" }).click();

  await expect(page.locator("form").getByText("Se o email existir, enviaremos um link para redefinir a senha.")).toBeVisible();
});
