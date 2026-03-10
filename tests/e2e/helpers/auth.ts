import { expect, type Page } from "@playwright/test";

export const authenticatedE2EEnabled = Boolean(process.env.E2E_EMAIL && process.env.E2E_PASSWORD);

export function getAuthenticatedE2ECredentials() {
  return {
    email: process.env.E2E_EMAIL ?? "",
    password: process.env.E2E_PASSWORD ?? "",
  };
}

export async function loginAsBusinessUser(page: Page) {
  const credentials = getAuthenticatedE2ECredentials();

  await page.goto("/login");
  await page.getByLabel("Email").fill(credentials.email);
  await page.getByLabel("Senha").fill(credentials.password);
  await page.getByRole("button", { name: "Entrar" }).click();
  await expect(page).toHaveURL(/\/dashboard/);
}

export function buildUniqueE2ELabel(prefix: string) {
  return `${prefix} ${Date.now()}`;
}
