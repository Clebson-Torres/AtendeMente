import { useState } from "react";
import { ShieldCheck, ShieldAlert, KeyRound, HardDriveDownload } from "lucide-react";
import { toast } from "../ui/Toast";
import { rotateDataKey, type RotationResult } from "../../lib/auth";

/**
 * Protege a chave dos prontuários com a senha.
 *
 * O que esta operação muda, em uma frase: hoje a chave que abre os prontuários é
 * derivada de um segredo guardado nesta máquina, então quem tiver acesso a esta
 * conta do Windows lê tudo sem saber a senha. Depois dela, a chave existe apenas
 * dentro de dois envelopes — um aberto pela senha, outro pelo código de
 * recuperação.
 *
 * Por que pede os dois segredos: a chave nova precisa nascer com os dois
 * envelopes, e um envelope só pode ser criado a partir do segredo em claro. Se
 * nascesse só com o da senha, esquecer a senha apagaria o prontuário.
 *
 * Por que é seguro executar: um backup completo é gravado e verificado ANTES de
 * qualquer registro ser tocado, e a chave antiga só é descartada depois de tudo
 * ser lido sob a nova. Se for interrompida, basta repetir — ela continua de onde
 * parou.
 */

interface Props {
  onDone: () => void;
  onCancel?: () => void;
}

export default function DataKeyRotation({ onDone, onCancel }: Props) {
  const [password, setPassword] = useState("");
  const [codigo, setCodigo] = useState("");
  const [erro, setErro] = useState("");
  const [rodando, setRodando] = useState(false);
  const [resultado, setResultado] = useState<RotationResult | null>(null);

  async function executar() {
    if (!password || !codigo) {
      setErro("Informe a senha e o código de recuperação.");
      return;
    }
    setErro("");
    setRodando(true);
    try {
      const r = await rotateDataKey(password, codigo);
      setPassword("");
      setCodigo("");
      setResultado(r);
      toast("Chave protegida pela sua senha.", "success");
    } catch (e: any) {
      setErro(e.message || "Não foi possível concluir. Nada foi alterado.");
    } finally {
      setRodando(false);
    }
  }

  if (resultado) {
    return (
      <div className="space-y-4">
        <div className="flex items-start gap-3">
          <ShieldCheck className="h-5 w-5 text-success flex-shrink-0 mt-0.5" aria-hidden="true" />
          <div>
            <h3 className="font-display font-semibold text-slate-900">Pronto</h3>
            <p className="text-sm text-slate-600">
              A chave dos prontuários agora depende da sua senha.
            </p>
          </div>
        </div>

        <dl className="text-sm text-slate-700 space-y-1">
          <div className="flex justify-between">
            <dt>Pacientes reprotegidos</dt>
            <dd className="font-medium">{resultado.patients}</dd>
          </div>
          <div className="flex justify-between">
            <dt>Prontuários de sessão</dt>
            <dd className="font-medium">{resultado.session_records}</dd>
          </div>
          <div className="flex justify-between">
            <dt>Anexos</dt>
            <dd className="font-medium">{resultado.files}</dd>
          </div>
        </dl>

        {resultado.safety_backup && (
          <div className="bg-muted rounded-xl p-3 flex gap-3">
            <HardDriveDownload className="h-4 w-4 text-slate-600 flex-shrink-0 mt-0.5" aria-hidden="true" />
            <div className="text-xs text-slate-700 space-y-1">
              <p>
                Um backup de segurança foi gravado antes da conversão, protegido pela sua
                senha:
              </p>
              <p className="font-mono break-all">{resultado.safety_backup}</p>
            </div>
          </div>
        )}

        <button
          type="button"
          onClick={onDone}
          className="w-full bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-medium cursor-pointer"
        >
          Concluir
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div className="flex items-start gap-3">
        <KeyRound className="h-5 w-5 text-slate-700 flex-shrink-0 mt-0.5" aria-hidden="true" />
        <div>
          <h3 className="font-display font-semibold text-slate-900">
            Proteger a chave com sua senha
          </h3>
          <p className="text-sm text-slate-600">
            Hoje ela depende apenas deste computador.
          </p>
        </div>
      </div>

      {erro && (
        <p role="alert" className="text-sm bg-destructive/10 text-destructive p-3 rounded-xl">
          {erro}
        </p>
      )}

      <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-xl flex gap-3">
        <ShieldAlert className="h-5 w-5 text-yellow-700 flex-shrink-0 mt-0.5" aria-hidden="true" />
        <div className="text-sm text-slate-800 space-y-2">
          <p>
            Hoje, quem tiver acesso a esta conta do computador consegue abrir os prontuários
            sem saber sua senha. Depois desta operação, isso deixa de ser possível.
          </p>
          <p>
            Em troca, <strong>a senha e o código de recuperação passam a ser
            indispensáveis</strong>. Sem um dos dois, não há como recuperar os dados — nem
            por suporte.
          </p>
          <p className="text-xs">
            Um backup completo é gravado e verificado antes de qualquer alteração. Se algo
            for interrompido no meio, basta repetir.
          </p>
        </div>
      </div>

      <div className="space-y-3">
        <div>
          <label className="block text-sm text-slate-700 mb-1" htmlFor="dk-senha">
            Sua senha
          </label>
          <input
            id="dk-senha"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={rodando}
            className="w-full rounded-xl border border-border px-3 py-2 text-sm"
          />
        </div>
        <div>
          <label className="block text-sm text-slate-700 mb-1" htmlFor="dk-codigo">
            Código de recuperação
          </label>
          <input
            id="dk-codigo"
            value={codigo}
            onChange={(e) => setCodigo(e.target.value)}
            disabled={rodando}
            placeholder="Os hífens são opcionais"
            className="w-full rounded-xl border border-border px-3 py-2 text-sm font-mono"
          />
        </div>
      </div>

      <div className="flex gap-3">
        <button
          type="button"
          onClick={executar}
          disabled={rodando}
          className="flex-1 bg-primary text-primary-foreground rounded-xl py-2.5 text-sm font-medium disabled:opacity-60 cursor-pointer"
        >
          {rodando ? "Reprotegendo os dados..." : "Proteger agora"}
        </button>
        {onCancel && !rodando && (
          <button
            type="button"
            onClick={onCancel}
            className="px-4 rounded-xl border border-border text-sm text-slate-700 cursor-pointer"
          >
            Agora não
          </button>
        )}
      </div>
    </div>
  );
}
