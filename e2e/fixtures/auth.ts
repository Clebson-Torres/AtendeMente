import type { Page } from "@playwright/test";
import { test as base } from "@playwright/test";

const API = "http://localhost:3001/api";

export interface TestUser {
  email: string;
  password: string;
  full_name: string;
  token: string;
  user_id: string;
}

/**
 * Register a new test user via the API and return credentials + token.
 * Emails are timestamped to avoid conflicts.
 */
export async function createTestUser(): Promise<TestUser> {
  const ts = Date.now();
  const email = `e2e-${ts}@test.com`;
  const password = "test123456";
  const full_name = "E2E Test User";

  const res = await fetch(`${API}/auth/register`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password, full_name }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Failed to create test user: ${res.status} ${body}`);
  }

  const json = await res.json();
  return {
    email,
    password,
    full_name,
    token: json.data.token,
    user_id: json.data.user_id,
  };
}

/**
 * Log in an existing user via the login page UI.
 * Navigates to /login, fills form, submits.
 * Returns after successful redirect to dashboard.
 */
export async function loginAs(page: Page, email: string, password: string) {
  await page.goto("/login");
  await page.waitForSelector('input[name="email"]');
  await page.fill('input[name="email"]', email);
  await page.fill('input[name="password"]', password);
  await page.click('button[type="submit"]');
  // Wait for navigation away from /login
  await page.waitForURL((url) => !url.pathname.includes("/login"), { timeout: 15000 });
}

/**
 * Extended test fixture that provides an authenticated page + user info.
 */
export const test = base.extend<{ user: TestUser; authPage: Page }>({
  user: [
    async ({}, use) => {
      const user = await createTestUser();
      await use(user);
    },
    { scope: "test", timeout: 30000 },
  ],
  authPage: [
    async ({ page, user }, use) => {
      await loginAs(page, user.email, user.password);
      await use(page);
    },
    { scope: "test", timeout: 30000 },
  ],
});

export { expect } from "@playwright/test";
