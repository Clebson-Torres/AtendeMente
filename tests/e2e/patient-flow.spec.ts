import { expect, test } from "@playwright/test";
import { authenticatedE2EEnabled, buildUniqueE2ELabel, loginAsBusinessUser } from "./helpers/auth";

test.describe("patient business flow", () => {
  test.skip(!authenticatedE2EEnabled, "Set E2E_EMAIL and E2E_PASSWORD to run authenticated e2e flows.");

  test("creates a patient, appointment, record, payment, receipt and export", async ({ page }) => {
    const patientName = buildUniqueE2ELabel("Paciente E2E");
    const patientEmail = `qa+${Date.now()}@atendemente.local`;
    const receiptName = `recibo-${Date.now()}.pdf`;

    await loginAsBusinessUser(page);
    await page.goto("/patients");

    await page.getByLabel("Nome completo").fill(patientName);
    await page.getByLabel("Data de nascimento").fill("12/06/1992");
    await page.getByLabel("Telefone").fill("(11) 98888-1234");
    await page.getByLabel("Telefone de emergencia").fill("(11) 97777-1234");
    await page.getByLabel("Email").fill(patientEmail);
    await page.getByLabel("Historico de saude").fill("TDAH");
    await page.keyboard.press("Enter");
    await page.getByLabel("Medicamentos em uso").fill("Sertralina 50mg");
    await page.getByLabel("Observacoes administrativas").fill("Paciente criado pelo smoke E2E.");
    await page.getByRole("button", { name: "Salvar paciente" }).click();

    await expect(page).toHaveURL(/\/patients\/.+/);
    await expect(page.getByRole("heading", { name: patientName })).toBeVisible();

    await page.locator("#novo-agendamento summary").click();
    await page.getByLabel("Data selecionada").fill("18/03/2026");
    await page.getByLabel("Horario de inicio").fill("10:00");
    await page.getByLabel("Horario final").fill("11:00");
    await page.getByLabel("Confirmacao").selectOption("confirmed");
    await page.getByLabel("Valor da sessao (R$)").fill("220,00");
    await page.getByRole("button", { name: "Criar atendimento" }).click();

    await expect(page).toHaveURL(/\/appointments\/.+/);
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
      mimeType: "application/pdf",
      buffer: Buffer.from("%PDF-1.4\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF"),
    });
    await page.getByRole("button", { name: "Enviar recibo" }).click();
    await expect(page.getByText("Arquivo anexado com seguranca.")).toBeVisible();
    await expect(page.getByText(receiptName)).toBeVisible();

    await page.getByRole("link", { name: patientName }).click();
    await expect(page).toHaveURL(/\/patients\/.+/);

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("link", { name: "Exportar dados" }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toMatch(/patient-.*\.zip/);
  });
});
