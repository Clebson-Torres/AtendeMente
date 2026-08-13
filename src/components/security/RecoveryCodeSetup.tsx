import { useState } from "react";
import { ShieldAlert, Copy, Check, Download, KeyRound } from "lucide-react";
import { toast } from "../ui/Toast";
import { downloadFile } from "../../lib/utils";
import { ackRecoveryCode, rotateRecoveryCode } from "../../lib/auth";

/**
 * Emite a segunda via da chave de dados e exige que ela seja anotada.
 *
 * Por que a re-digitação é obrigatória e não há botão de "pular":
 *
 * O código de recuperação é a única forma de abrir os prontuários sem a senha.
 * Depois que a chave for rotacionada, esquecer a senha e não ter o código
 * significa que o histórico clínico de todos os pacientes fica inacessível —
 * para sempre, e sem caminho de suporte, porque ninguém mais consegue abrir
 * aquela chave. Não é um inconveniente recuperável.
 *
 * "Baixei o arquivo" não prova nada: o download pode ir para a pasta errada, o
 * arquivo pode ser apagado numa limpeza, e a pessoa só descobre no dia em que
 * precisa. Digitar de volta prova que o código saiu da tela e chegou em algum
 * lugar que ela consegue ler.
 *
 * O código anterior deixa de valer no instante em que o novo é emitido, então
 * este fluxo não pode ser interrompido no meio sem consequência — por isso o
 * aviso aparece ANTES de emitir, e não depois.
 */

type Etapa = "aviso" | "senha" | "guardar" | "confirmar";

interface Props {
  onDone: () => void;
  onCancel?: () => void;
}

function normalizar(raw: string): string {
  return raw.replace(/[^0-9a-zA-Z]/g, "").toUpperCase();
}

export default function RecoveryCodeSetup({ onDone, onCancel }: Props) {
  const [etapa, setEtapa] = useState<Etapa>("aviso");
  const [password, setPassword] = useState("");
  const [codigo, setCodigo] = useState("");
  const [userId, setUserId] = useState("");
  const [digitado, setDigitado] = useState("");
  const [erro, setErro] = useState("");
  const [carregando, setCarregando] = useState(false);
  const [copiado, setCopiado] = useState(false);

  async function emitir() {
    if (!password) {
      setErro("Informe sua senha.");
      return;
    }
    setErro("");
    setCarregando(true);
    try {
      const r = await rotateRecoveryCode(password);
      setCodigo(r.recovery_secret);
      setUserId(r.user_id);
      setPassword("");
      setEtapa("guardar");
    } catch (e: any) {
      setErro(e.message || "Não foi possível gerar o código.");
    } finally {
      setCarregando(false);
    }
  }

  async function confirmar() {
    if (normalizar(digitado) !== normalizar(codigo)) {
      setErro("O código digitado não confere. Confira e tente de novo.");
      return;
    }
    setErro("");
    setCarregando(true);
    try {
      await ackRecoveryCode();
      toast("Código de recuperação guardado.", "success");
      onDone();
    } catch (e: any) {
      setErro(e.message || "Erro ao confirmar.");
    } finally {
      setCarregando(false);
    }
  }

  async function copiar() {
    try {
      await navigator.clipboard.writeText(codigo);
      setCopiado(true);
      setTimeout(() => setCopiado(false), 2000);
    } catch {
      toast("Não foi possível copiar. Anote manualmente.", "error");
    }
  }

  function baixar() {
    const conteudo = JSON.stringify(
      { version: 1, user_id: userId, recovery_secret: codigo },
      null,
      2
    );
    downloadFile(
      new Blob([conteudo], { type: "application/json" }),
      `atendemente-recuperacao-${userId.slice(0, 8)}.json`
    );
  }

  return (
    <div className="space-y-5">
      <div className="flex items-start gap-3">
        <KeyRound className="h-5 w-5 text-slate-700 flex-shrink-0 mt-0.5" aria-hidden="true" />
        <div>
          <h3 className="font-display font-semibold text-slate-900">Código de recuperação</h3>
          <p className="text-sm text-slate-600">
            A única forma de abrir seus prontuários sem a senha.
          </p>
        </div>
      </div>

      {erro && (
        <p role="alert" className="text-sm bg-destructive/10 text-destructive p-3 rounded-xl">
          {erro}
        </p>
      )}

      {etapa === "aviso" && (
        <>
          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-xl flex gap-3">
            <ShieldAlert className="h-5 w-5 text-yellow-700 flex-shrink-0 mt-0.5" aria-hidden="true" />
            <div className="text-sm text-slate-800 space-y-2">
              <p>
                Sua conta ainda não tem uma segunda via da chave que abre os prontuários.
                Vamos gerar uma agora.
              </p>
              <p>
                <strong>O código atual deixará de funcionar</strong> assim que o novo for
                gerado. Tenha onde anotá-lo antes de continuar — você precisará digitá-lo de
                volta para concluir.
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <button
              type="button"
              onClick={() => setEtapa("senha")}
              className="flex-1 bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-medium cursor-pointer"
            >
              Estou pronta para anotar
            </button>
            {onCancel && (
              <button
                type="button"
                onClick={onCancel}
                className="px-4 rounded-xl border border-border text-sm text-slate-700 cursor-pointer"
              >
                Agora não
              </button>
            )}
          </div>
        </>
      )}

      {etapa === "senha" && (
        <>
          <label className="block text-sm text-slate-700" htmlFor="rc-senha">
            Digite sua senha para liberar a chave
          </label>
          <input
            id="rc-senha"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && emitir()}
            className="w-full rounded-xl border border-border px-3 py-2 text-sm"
          />
          <button
            type="button"
            onClick={emitir}
            disabled={carregando}
            className="w-full bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-medium disabled:opacity-60 cursor-pointer"
          >
            {carregando ? "Gerando..." : "Gerar código"}
          </button>
        </>
      )}

      {(etapa === "guardar" || etapa === "confirmar") && (
        <>
          <div className="bg-slate-900 text-slate-50 rounded-xl p-4 text-center">
            <p className="font-mono text-base tracking-wider break-all select-all">{codigo}</p>
          </div>

          <div className="flex gap-2">
            <button
              type="button"
              onClick={copiar}
              className="flex-1 inline-flex items-center justify-center gap-2 rounded-xl border border-border py-2 text-sm cursor-pointer"
            >
              {copiado ? <Check className="h-4 w-4" aria-hidden="true" /> : <Copy className="h-4 w-4" aria-hidden="true" />}
              {copiado ? "Copiado" : "Copiar"}
            </button>
            <button
              type="button"
              onClick={baixar}
              className="flex-1 inline-flex items-center justify-center gap-2 rounded-xl border border-border py-2 text-sm cursor-pointer"
            >
              <Download className="h-4 w-4" aria-hidden="true" />
              Baixar arquivo
            </button>
          </div>

          <div className="space-y-2">
            <label className="block text-sm text-slate-700" htmlFor="rc-confirma">
              Digite o código de volta para confirmar que o guardou
            </label>
            <input
              id="rc-confirma"
              value={digitado}
              onChange={(e) => {
                setDigitado(e.target.value);
                if (etapa === "guardar") setEtapa("confirmar");
              }}
              onKeyDown={(e) => e.key === "Enter" && confirmar()}
              placeholder="Os hífens são opcionais"
              className="w-full rounded-xl border border-border px-3 py-2 text-sm font-mono"
            />
            <button
              type="button"
              onClick={confirmar}
              disabled={carregando || normalizar(digitado).length === 0}
              className="w-full bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-medium disabled:opacity-60 cursor-pointer"
            >
              {carregando ? "Confirmando..." : "Confirmar e concluir"}
            </button>
            <p className="text-xs text-slate-600">
              Guarde fora do computador — num papel, num cofre de senhas, ou impresso. Se ele
              ficar só nesta máquina, um problema aqui leva o código junto com os dados.
            </p>
          </div>
        </>
      )}
    </div>
  );
}
