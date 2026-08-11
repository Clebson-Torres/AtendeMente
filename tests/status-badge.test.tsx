import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import StatusBadge from "../src/components/ui/StatusBadge";

describe("StatusBadge", () => {
  // A UI e toda em portugues; um status sem traducao apareceria como o valor cru
  // do banco ("active", "no_show") na tela do usuario.
  it.each([
    ["active", "Ativo"],
    ["inactive", "Inativo"],
    ["scheduled", "Agendado"],
    ["confirmed", "Confirmado"],
    ["unconfirmed", "Nao confirmado"],
    ["pending", "Pendente"],
    ["cancelled", "Cancelado"],
    ["completed", "Concluido"],
    ["no_show", "Nao compareceu"],
    ["paid", "Pago"],
    ["unpaid", "Nao pago"],
    ["partial", "Parcial"],
  ])("traduz %s para %s", (status, esperado) => {
    const { unmount } = render(<StatusBadge status={status} />);
    expect(screen.getByText(esperado)).toBeInTheDocument();
    unmount();
  });

  it("cobre todos os status usados pelo schema do banco", () => {
    // Espelha os CHECK constraints das migrations: se um status novo for
    // adicionado no banco sem traducao, este teste falha.
    const doBanco = [
      "active", "inactive",                                    // patients.status
      "scheduled", "completed", "cancelled", "no_show",          // appointments.status
      "unconfirmed", "confirmed",                               // confirmation_status
      "pending", "paid",                                        // payments.status
    ];
    for (const s of doBanco) {
      const { unmount } = render(<StatusBadge status={s} />);
      expect(
        screen.queryByText(s),
        `status ${s} apareceu sem traducao`
      ).not.toBeInTheDocument();
      unmount();
    }
  });

  it("nao quebra com status desconhecido", () => {
    render(<StatusBadge status="algo_novo" />);
    // Sem traducao cai no proprio valor, mas nao deve lancar.
    expect(screen.getByText("algo_novo")).toBeInTheDocument();
  });

  it("normaliza maiusculas", () => {
    render(<StatusBadge status="ACTIVE" />);
    expect(screen.getByText("Ativo")).toBeInTheDocument();
  });
});
