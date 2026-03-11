import { expect, type Page } from "@playwright/test";
import postgres from "postgres";

export const authenticatedE2EEnabled = Boolean(process.env.E2E_EMAIL && process.env.E2E_PASSWORD);

export function getAuthenticatedE2ECredentials() {
  return {
    email: process.env.E2E_EMAIL ?? "",
    password: process.env.E2E_PASSWORD ?? "",
  };
}

async function resetLoginRateLimit(email: string) {
  const databaseUrl = process.env.DATABASE_URL;

  if (!databaseUrl) {
    return;
  }

  const sql = postgres(databaseUrl, {
    max: 1,
    prepare: false,
  });

  try {
    await sql`
      delete from request_limits
      where scope = 'auth:login'
        and identifier = ${email.toLowerCase()}
    `;
  } finally {
    await sql.end({ timeout: 1 });
  }
}

export async function loginAsBusinessUser(page: Page) {
  const credentials = getAuthenticatedE2ECredentials();
  let lastError: unknown;

  await resetLoginRateLimit(credentials.email);

  for (let attempt = 1; attempt <= 2; attempt += 1) {
    await page.goto("/login");
    await page.getByLabel("Email").fill(credentials.email);
    await page.getByLabel("Senha").fill(credentials.password);
    await page.getByRole("button", { name: "Entrar" }).click();

    try {
      await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });
      return;
    } catch (error) {
      lastError = error;
    }
  }

  throw lastError;
}

export function buildUniqueE2ELabel(prefix: string) {
  return `${prefix} ${Date.now()}`;
}
