import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";

// vi.mock e hoisted acima das declaracoes do modulo, entao os mocks precisam
// existir antes — vi.hoisted resolve isso.
const { getConfig, getPasswordStatus, setPassword, clearPassword } = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getPasswordStatus: vi.fn(),
  setPassword: vi.fn(),
  clearPassword: vi.fn(),
}));

vi.mock("../src/lib/api", () => ({
  api: {
    backup: { getConfig, getPasswordStatus, setPassword, clearPassword },
  },
}));
vi.mock("../src/components/ui/Toast", () => ({ toast: vi.fn() }));
vi.mock("../src/lib/utils", async (orig) => ({
  ...(await orig<typeof import("../src/lib/utils")>()),
  downloadFile: vi.fn(),
}));

import Settings from "../src/pages/Settings";

beforeEach(() => {
  vi.clearAllMocks();
  getConfig.mockResolvedValue({ frequency: "daily", last_backup_at: "2026-07-31T18:25:38.931Z" });
  getPasswordStatus.mockResolvedValue({ configured: false });
  setPassword.mockResolvedValue(undefined);
  clearPassword.mockResolvedValue(undefined);
});

describe("Settings — senha do backup automatico", () => {
  it("avisa que nenhum backup sera gerado quando falta senha", async () => {
    render(<Settings />);

    await waitFor(() =>
      expect(screen.getByText(/Nenhum backup automatico sera gerado/i)).toBeInTheDocument()
    );
    expect(screen.getByRole("button", { name: /Definir senha/i })).toBeInTheDocument();
    // A secao de backup automatico nao deve alegar um "ultimo backup" enquanto
    // nenhum backup pode ser gerado. (A secao de backup MANUAL tem o seu proprio,
    // legitimo, por isso a contagem e 1 e nao 0.)
    expect(screen.getAllByText(/Ultimo backup:/i)).toHaveLength(1);
  });

  it("mostra o estado configurado e nao expoe a senha", async () => {
    getPasswordStatus.mockResolvedValue({ configured: true });
    render(<Settings />);

    await waitFor(() => expect(screen.getByText(/Senha configurada/i)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /Alterar senha/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Remover senha/i })).toBeInTheDocument();
    expect(screen.queryByText(/Nenhum backup automatico/i)).not.toBeInTheDocument();
  });

  it("salva a senha e passa a exibir o estado configurado", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await waitFor(() => screen.getByRole("button", { name: /Definir senha/i }));

    await user.click(screen.getByRole("button", { name: /Definir senha/i }));

    const campos = screen.getAllByPlaceholderText(/Senha|Confirmar/i);
    await user.type(campos[0], "senha-de-backup-forte");
    await user.type(campos[1], "senha-de-backup-forte");
    await user.click(screen.getByRole("button", { name: /Salvar senha/i }));

    await waitFor(() => expect(setPassword).toHaveBeenCalledWith("senha-de-backup-forte"));
    await waitFor(() => expect(screen.getByText(/Senha configurada/i)).toBeInTheDocument());
  });

  it("bloqueia senha curta e senhas que nao conferem", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await waitFor(() => screen.getByRole("button", { name: /Definir senha/i }));
    await user.click(screen.getByRole("button", { name: /Definir senha/i }));

    const campos = screen.getAllByPlaceholderText(/Senha|Confirmar/i);
    const salvar = screen.getByRole("button", { name: /Salvar senha/i });

    await user.type(campos[0], "curta");
    await user.type(campos[1], "curta");
    expect(salvar).toBeDisabled();

    await user.clear(campos[0]);
    await user.clear(campos[1]);
    await user.type(campos[0], "senha-de-backup-forte");
    await user.type(campos[1], "senha-diferente-1234");
    expect(salvar).toBeDisabled();

    await user.clear(campos[1]);
    await user.type(campos[1], "senha-de-backup-forte");
    expect(salvar).toBeEnabled();
    expect(setPassword).not.toHaveBeenCalled();
  });

  it("remover a senha volta ao estado de aviso", async () => {
    const user = userEvent.setup();
    getPasswordStatus.mockResolvedValue({ configured: true });
    render(<Settings />);
    await waitFor(() => screen.getByRole("button", { name: /Remover senha/i }));

    await user.click(screen.getByRole("button", { name: /Remover senha/i }));

    await waitFor(() => expect(clearPassword).toHaveBeenCalled());
    await waitFor(() =>
      expect(screen.getByText(/Nenhum backup automatico sera gerado/i)).toBeInTheDocument()
    );
  });

  it("nao mostra a secao de senha quando o backup automatico esta desligado", async () => {
    getConfig.mockResolvedValue({ frequency: "never", last_backup_at: null });
    render(<Settings />);

    await waitFor(() =>
      expect(screen.getByRole("heading", { name: /Backup Automatico/i })).toBeInTheDocument()
    );
    expect(screen.queryByText(/Nenhum backup automatico sera gerado/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Definir senha/i })).not.toBeInTheDocument();
  });

  it("nao oferece mais o toggle de acesso mobile", async () => {
    render(<Settings />);
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: /Backup Automatico/i })).toBeInTheDocument()
    );
    expect(screen.queryByText(/Acesso Mobile/i)).not.toBeInTheDocument();
  });
});
