import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";
import Modal from "../src/components/ui/Modal";

/**
 * Reproduz o uso real: `onClose` e uma arrow inline (recriada a cada render) e o
 * input e controlado, entao cada tecla re-renderiza o pai. Foi essa combinacao
 * que fazia o Modal roubar o foco do input a cada caractere.
 */
function PasswordModalShell() {
  const [open, setOpen] = useState(true);
  const [password, setPassword] = useState("");
  return (
    <>
      <div data-testid="valor">{password}</div>
      <Modal open={open} onClose={() => setOpen(false)} title="Senha do Backup" size="sm">
        <input
          aria-label="Senha"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Modal>
    </>
  );
}

describe("Modal", () => {
  it("mantem o foco no input enquanto se digita", async () => {
    const user = userEvent.setup();
    render(<PasswordModalShell />);

    const input = screen.getByLabelText("Senha");
    await user.click(input);
    // Sem digitar caractere por caractere o bug nao aparece: `fill`/`type` em um
    // unico passo mascarava isso nos testes e2e.
    await user.type(input, "senha-de-backup-forte");

    expect(input).toHaveValue("senha-de-backup-forte");
    expect(screen.getByTestId("valor")).toHaveTextContent("senha-de-backup-forte");
    expect(document.activeElement).toBe(input);
  });

  it("ainda fecha com Escape depois de digitar", async () => {
    const user = userEvent.setup();
    render(<PasswordModalShell />);

    const input = screen.getByLabelText("Senha");
    await user.click(input);
    await user.type(input, "abc");
    expect(screen.getByText("Senha do Backup")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByText("Senha do Backup")).not.toBeInTheDocument();
  });

  it("devolve o foco a quem abriu o modal ao fechar", async () => {
    const user = userEvent.setup();

    function Shell() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Definir senha</button>
          <Modal open={open} onClose={() => setOpen(false)} title="Senha" size="sm">
            <input aria-label="Campo" />
          </Modal>
        </>
      );
    }

    render(<Shell />);
    const trigger = screen.getByRole("button", { name: "Definir senha" });
    await user.click(trigger);
    expect(screen.getByLabelText("Campo")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(trigger);
  });
});
