import { expect, test } from "@playwright/test";
import { authenticatedE2EEnabled, buildUniqueE2ELabel, loginAsBusinessUser } from "./helpers/auth";

test.describe("patient import flow", () => {
  test.skip(!authenticatedE2EEnabled, "Set E2E_EMAIL and E2E_PASSWORD to run authenticated e2e flows.");

  test("imports patients from CSV preview and commit", async ({ page }) => {
    const importName = buildUniqueE2ELabel("Importacao CSV");
    const csv = [
      "nome,telefone,email,nascimento,medicamentos,historico de saude,observacoes",
      `${importName},(11) 95555-1122,${Date.now()}@atendemente.local,07/03/1991,Vitamina D,TDAH,Importado pelo teste`,
    ].join("\n");

    await loginAsBusinessUser(page);
    await page.goto("/patients");

    await page.locator("#patients-import-file").setInputFiles({
      name: "patients-e2e.csv",
      mimeType: "text/csv",
      buffer: Buffer.from(csv),
    });
    await page.getByRole("button", { name: "Gerar preview" }).click();
    await expect(page.getByText("Validas", { exact: true })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText("1").first()).toBeVisible();

    await page.getByRole("button", { name: "Importar pacientes" }).click();
    await expect(page.getByText("1 pacientes importados com sucesso.")).toBeVisible({ timeout: 20_000 });
    await expect(page.getByRole("button", { name: "Importar pacientes" })).toBeDisabled();
    await expect(page.getByText("Nenhum arquivo selecionado")).toBeVisible();
    await expect(page.getByText("Validas", { exact: true })).not.toBeVisible();

    await page.getByPlaceholder("Buscar por nome, telefone ou email").fill(importName);
    await page.getByRole("button", { name: "Buscar" }).click();
    await expect(page.getByText(importName, { exact: true })).toBeVisible();
  });
});
