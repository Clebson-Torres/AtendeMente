import { test, expect } from "../fixtures/auth";
import type { Page } from "@playwright/test";

async function logout(page: Page) {
  const logoutBtn = page.locator("text=Sair");
  if (await logoutBtn.isVisible()) {
    await logoutBtn.click();
    await page.waitForURL("/login", { timeout: 10000 });
  }
}

test.describe("Authentication", () => {
  test("login page loads with form fields", async ({ page }) => {
    await page.goto("/login");
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toContainText("Entrar");
  });

  test("login with valid credentials redirects to dashboard", async ({ page, user }) => {
    await page.goto("/login");
    await page.fill('input[name="email"]', user.email);
    await page.fill('input[name="password"]', user.password);
    await page.click('button[type="submit"]');
    await page.waitForURL((url) => !url.pathname.includes("/login"), { timeout: 15000 });
    expect(page.url()).not.toContain("/login");
  });

  test("login with invalid credentials shows error", async ({ page }) => {
    await page.goto("/login");
    await page.fill('input[name="email"]', "wrong@email.com");
    await page.fill('input[name="password"]', "wrongpassword");
    await page.click('button[type="submit"]');
    // Should show error toast or message
    await expect(page.locator("text=Email ou senha").first()).toBeVisible({ timeout: 10000 });
  });

  test("protected route redirects to /login when unauthenticated", async ({ page }) => {
    await page.goto("/patients");
    await page.waitForURL("/login", { timeout: 10000 });
    expect(page.url()).toContain("/login");
  });

  test("logout clears session and redirects to login", async ({ authPage, user }) => {
    await authPage.waitForURL((url) => !url.pathname.includes("/login"), { timeout: 15000 });
    const logoutBtn = authPage.locator("text=Sair");
    if (await logoutBtn.isVisible()) {
      await logoutBtn.click();
      await authPage.waitForURL("/login", { timeout: 10000 });
      expect(authPage.url()).toContain("/login");
    }
  });

  test("after logout, protected routes redirect to login", async ({ authPage, user }) => {
    await authPage.waitForURL((url) => !url.pathname.includes("/login"), { timeout: 15000 });
    // Logout
    const logoutBtn = authPage.locator("text=Sair");
    if (await logoutBtn.isVisible()) {
      await logoutBtn.click();
      await authPage.waitForURL("/login", { timeout: 10000 });
    }
    // Try accessing protected route
    await authPage.goto("/payments");
    await authPage.waitForURL("/login", { timeout: 10000 });
    expect(authPage.url()).toContain("/login");
  });

  test("sidebar shows user email and nav links when authenticated", async ({ authPage, user }) => {
    await authPage.waitForSelector("text=Visão geral", { timeout: 10000 });
    await expect(authPage.locator(`text=${user.email}`).first()).toBeVisible();
    await expect(authPage.locator("text=Visão geral")).toBeVisible();
    await expect(authPage.locator("text=Agenda")).toBeVisible();
    await expect(authPage.locator("text=Pacientes")).toBeVisible();
    await expect(authPage.locator("text=Financeiro")).toBeVisible();
  });

  test("sidebar navigation works correctly", async ({ authPage }) => {
    await authPage.waitForSelector("text=Visão geral", { timeout: 10000 });

    await authPage.locator("text=Pacientes").click();
    await authPage.waitForURL("/patients", { timeout: 10000 });
    expect(authPage.url()).toContain("/patients");

    await authPage.locator("text=Agenda").click();
    await authPage.waitForURL("/appointments", { timeout: 10000 });
    expect(authPage.url()).toContain("/appointments");

    await authPage.locator("text=Financeiro").click();
    await authPage.waitForURL("/payments", { timeout: 10000 });
    expect(authPage.url()).toContain("/payments");

    await authPage.locator("text=Visão geral").click();
    await authPage.waitForURL("/", { timeout: 10000 });
  });
});
