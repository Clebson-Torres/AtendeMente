import { expect, test } from "@playwright/test";
import { authenticatedE2EEnabled, buildUniqueE2ELabel, loginAsBusinessUser } from "./helpers/auth";

test.describe("patient business flow", () => {
  test.skip(!authenticatedE2EEnabled, "Set E2E_EMAIL and E2E_PASSWORD to run authenticated e2e flows.");

  test("creates a patient, appointment, record, payment, receipt and export", async ({ page }) => {
    test.setTimeout(120_000);

    const patientName = buildUniqueE2ELabel("Paciente E2E");
    const patientEmail = `qa+${Date.now()}@atendemente.local`;
    const chartNumber = `PR-${Date.now()}`;
    const updatedChartNumber = `${chartNumber}-A`;
    const receiptName = `recibo-${Date.now()}.png`;

    await loginAsBusinessUser(page);
    await page.goto("/patients");

    await page.getByLabel("Nome completo").fill(patientName);
    await page.getByLabel("Numero do prontuario").fill(chartNumber);
    await page.getByLabel("Data de nascimento").fill("12/06/1992");
    await page.getByLabel("Telefone", { exact: true }).fill("(11) 98888-1234");
    await page.getByLabel("Telefone de emergencia").fill("(11) 97777-1234");
    await page.getByLabel("Email").fill(patientEmail);
    await page.getByLabel("Historico de saude").fill("TDAH");
    await page.keyboard.press("Enter");
    await page.getByLabel("Medicamentos em uso").fill("Sertralina 50mg");
    await page.getByLabel("Observacoes administrativas").fill("Paciente criado pelo smoke E2E.");
    await page.getByRole("button", { name: "Salvar paciente" }).click();

    await expect(page.getByText("Paciente cadastrado com sucesso.")).toBeVisible({ timeout: 10_000 });
    const patientHeading = page.getByRole("heading", { name: patientName });

    try {
      await expect(patientHeading).toBeVisible({ timeout: 12_000 });
    } catch {
      const patientRow = page.getByRole("row", { name: new RegExp(patientName) });
      await expect(patientRow).toBeVisible({ timeout: 20_000 });
      await patientRow.getByRole("link", { name: "Abrir ficha" }).click();
    }

    await expect(patientHeading).toBeVisible({ timeout: 20_000 });
    await expect(page.getByText(`Prontuario: ${chartNumber}`)).toBeVisible();
    await expect(page.getByText(chartNumber)).toHaveCount(2);

    await page.getByRole("button", { name: "Editar Numero do prontuario" }).click();
    await page.locator('input[placeholder="Ex.: PR-2026-001"]').fill(updatedChartNumber);
    await page.getByRole("button", { name: "Salvar" }).click();
    await expect(page.getByText("Campo atualizado com sucesso.")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(`Prontuario: ${updatedChartNumber}`)).toBeVisible({ timeout: 20_000 });
    await expect(page.getByText(updatedChartNumber)).toHaveCount(2);

    await page.locator("#novo-agendamento summary").click();
    await page.getByLabel("Data selecionada").fill("18/03/2026");
    await page.getByLabel("Horario de inicio").fill("10:00");
    await page.getByLabel("Horario final").fill("11:00");
    await page.getByLabel("Confirmacao").selectOption("confirmed");
    await page.getByLabel("Valor da sessao (R$)").fill("220,00");
    await page.getByRole("button", { name: "Criar atendimento" }).click();
    await expect(page.getByText("Atendimento criado com sucesso.")).toBeVisible({ timeout: 20_000 });
    const redirectedToAppointment = await page
      .waitForURL(/\/appointments\/.+/, { timeout: 30_000 })
      .then(() => true)
      .catch(() => false);

    if (!redirectedToAppointment) {
      await expect(page).toHaveURL(/\/patients\/.+/, { timeout: 20_000 });
      const appointmentLink = page.locator('a[href^="/appointments/"]').first();
      await expect(appointmentLink).toBeVisible({ timeout: 20_000 });
      await appointmentLink.click();
      await expect(page).toHaveURL(/\/appointments\/.+/, { timeout: 20_000 });
    }

    await expect(page.getByRole("heading", { name: patientName })).toBeVisible();

    await page.getByLabel("Registro do atendimento").fill("Registro criptografado salvo pelo fluxo E2E.");
    await page.getByRole("button", { name: "Salvar registro criptografado" }).click();
    await expect(page.getByText("Registro salvo com seguranca.")).toBeVisible();

    await page.locator("#payment-status").selectOption("paid");
    await page.locator("#payment-method").selectOption("pix");
    await page.locator("#payment-date-input").fill("18/03/2026");
    await page.locator("#payment-value-input").fill("220,00");
    await page.locator("#payment-notes").fill("Pagamento confirmado no fluxo E2E.");
    await page.getByRole("button", { name: "Salvar pagamento" }).click();
    await expect(page.getByText("Pagamento salvo com sucesso.")).toBeVisible();

    await page.getByLabel("Arquivo do recibo").setInputFiles({
      name: receiptName,
      mimeType: "image/png",
      buffer: Buffer.from(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z0uoAAAAASUVORK5CYII=",
        "base64",
      ),
    });
    await page.getByRole("button", { name: "Enviar recibo" }).click();
    await expect(page.getByText(receiptName)).toBeVisible({ timeout: 20_000 });

    await page.getByRole("link", { name: patientName }).click();
    await expect(page).toHaveURL(/\/patients\/.+/);
    await expect(page.getByText(`Prontuario: ${updatedChartNumber}`)).toBeVisible({ timeout: 20_000 });
    await expect(page.getByText(updatedChartNumber)).toHaveCount(2);

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("link", { name: "Exportar dados" }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toMatch(/patient-.*\.zip/);
  });
});
