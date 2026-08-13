import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import RecoveryCodeSetup from "../src/components/security/RecoveryCodeSetup";

const rotateRecoveryCode = vi.fn();
const ackRecoveryCode = vi.fn();

vi.mock("../src/lib/auth", () => ({
  rotateRecoveryCode: (...a: unknown[]) => rotateRecoveryCode(...a),
  ackRecoveryCode: (...a: unknown[]) => ackRecoveryCode(...a),
}));
vi.mock("../src/components/ui/Toast", () => ({ toast: vi.fn() }));
vi.mock("../src/lib/utils", () => ({ downloadFile: vi.fn() }));

const CODIGO = "728E-F79C-AB3B-B46C-E982-34DC-8ADA-7847";

async function chegarNaTelaDoCodigo() {
  rotateRecoveryCode.mockResolvedValue({ user_id: "u-1", recovery_secret: CODIGO });
  const onDone = vi.fn();
  render(<RecoveryCodeSetup onDone={onDone} />);
  fireEvent.click(screen.getByRole("button", { name: /pronta para anotar/i }));
  fireEvent.change(screen.getByLabelText(/senha/i), { target: { value: "minha-senha" } });
  fireEvent.click(screen.getByRole("button", { name: /gerar c[óo]digo/i }));
  await screen.findByText(CODIGO);
  return { onDone };
}

describe("RecoveryCodeSetup", () => {
  beforeEach(() => {
    rotateRecoveryCode.mockReset();
    ackRecoveryCode.mockReset();
  });

  /**
   * O código anterior deixa de valer no instante em que o novo é emitido. Se o
   * aviso viesse depois, a pessoa descobriria o compromisso já assumido.
   */
  it("avisa que o código atual deixará de valer ANTES de emitir", () => {
    render(<RecoveryCodeSetup onDone={vi.fn()} />);
    expect(screen.getByText(/deixará de funcionar/i)).toBeInTheDocument();
    expect(rotateRecoveryCode).not.toHaveBeenCalled();
  });

  it("exige a senha para emitir", async () => {
    render(<RecoveryCodeSetup onDone={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /pronta para anotar/i }));
    fireEvent.click(screen.getByRole("button", { name: /gerar c[óo]digo/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/informe sua senha/i);
    expect(rotateRecoveryCode).not.toHaveBeenCalled();
  });

  /**
   * O ponto central da tela: sem a re-digitação, "baixei o arquivo" não prova
   * nada — o download pode ir para a pasta errada ou ser apagado numa limpeza,
   * e a pessoa só descobre no dia em que precisa. Aqui isso é irreversível.
   */
  it("não conclui enquanto o código digitado não confere", async () => {
    const { onDone } = await chegarNaTelaDoCodigo();

    fireEvent.change(screen.getByLabelText(/digite o c[óo]digo de volta/i), {
      target: { value: "0000-0000-0000-0000-0000-0000-0000-0000" },
    });
    fireEvent.click(screen.getByRole("button", { name: /confirmar e concluir/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/n[ãa]o confere/i);
    expect(ackRecoveryCode).not.toHaveBeenCalled();
    expect(onDone).not.toHaveBeenCalled();
  });

  it("conclui quando o código confere, aceitando sem hífens e em minúsculas", async () => {
    ackRecoveryCode.mockResolvedValue(undefined);
    const { onDone } = await chegarNaTelaDoCodigo();

    fireEvent.change(screen.getByLabelText(/digite o c[óo]digo de volta/i), {
      target: { value: CODIGO.replace(/-/g, "").toLowerCase() },
    });
    fireEvent.click(screen.getByRole("button", { name: /confirmar e concluir/i }));

    await waitFor(() => expect(ackRecoveryCode).toHaveBeenCalledTimes(1));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("mostra o erro do servidor quando a senha está incorreta", async () => {
    rotateRecoveryCode.mockRejectedValue(new Error("Senha incorreta."));
    render(<RecoveryCodeSetup onDone={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /pronta para anotar/i }));
    fireEvent.change(screen.getByLabelText(/senha/i), { target: { value: "errada" } });
    fireEvent.click(screen.getByRole("button", { name: /gerar c[óo]digo/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/senha incorreta/i);
    expect(ackRecoveryCode).not.toHaveBeenCalled();
  });
});
