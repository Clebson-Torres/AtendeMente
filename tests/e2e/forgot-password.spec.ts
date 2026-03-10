import { expect, test } from "@playwright/test";

test("renders forgot-password recovery experience", async ({ page }) => {
  await page.goto("/forgot-password");

  await expect(page.getByRole("heading", { name: "Recuperar acesso" })).toBeVisible();
  await expect(page.getByText("Informe seu email para receber o link de redefinicao de senha.")).toBeVisible();
  await expect(page.getByLabel("Email")).toBeVisible();
  await expect(page.getByRole("button", { name: "Enviar link de redefinicao" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Voltar para o login" })).toBeVisible();
});
