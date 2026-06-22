import { useState } from "react";
import { useNavigate, Navigate, Link } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { login, recoverPassword, resetPassword } from "../lib/auth";
import { useAuth } from "../App";
import { loginSchema, resetPasswordSchema, type LoginInput } from "../lib/schemas";
import FieldError from "../components/ui/FieldError";

const RECOVERY_CODE_REGEX = /^[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}$/;

export default function Login() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<"login" | "recover" | "reset">("login");
  const [recoverMethod, setRecoverMethod] = useState<"file" | "manual">("file");
  const [recoveryFile, setRecoveryFile] = useState<{ user_id: string; recovery_secret: string } | null>(null);
  const [manualEmail, setManualEmail] = useState("");
  const [manualCode, setManualCode] = useState("");
  const [resetToken, setResetToken] = useState("");
  const [successMsg, setSuccessMsg] = useState("");

  const { register, handleSubmit, formState: { errors } } = useForm<LoginInput>({
    resolver: zodResolver(loginSchema),
  });

  const { register: registerReset, handleSubmit: handleResetSubmit, formState: { errors: resetErrors } } = useForm({
    resolver: zodResolver(resetPasswordSchema),
  });

  if (user) return <Navigate to="/" replace />;

  async function onLogin(data: LoginInput) {
    setError("");
    setLoading(true);
    try {
      await login(data.email, data.password);
      navigate("/");
    } catch (err: any) {
      setError(err.message || "Erro ao fazer login");
    } finally {
      setLoading(false);
    }
  }

  function handleSelectFile() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const data = JSON.parse(text);
        if (!data.user_id || !data.recovery_secret) throw new Error("Arquivo inválido");
        setRecoveryFile(data);
        setError("");
      } catch {
        setError("Arquivo de recuperação inválido.");
      }
    };
    input.click();
  }

  async function handleRecover() {
    if (!recoveryFile) return;
    setError("");
    setSuccessMsg("");
    setLoading(true);
    try {
      const token = await recoverPassword({ user_id: recoveryFile.user_id, recovery_secret: recoveryFile.recovery_secret });
      setResetToken(token);
      setMode("reset");
      setSuccessMsg("Chave verificada! Crie uma nova senha.");
    } catch (err: any) {
      setError(err.message || "Erro ao recuperar senha");
    } finally {
      setLoading(false);
    }
  }

  async function handleManualRecover() {
    const code = manualCode.trim().toUpperCase();
    if (!RECOVERY_CODE_REGEX.test(code)) {
      setError("O código de recuperação deve estar no formato XXXX-XXXX-XXXX-XXXX.");
      return;
    }
    const email = manualEmail.trim().toLowerCase();
    if (!email) {
      setError("Informe seu email.");
      return;
    }
    setError("");
    setSuccessMsg("");
    setLoading(true);
    try {
      const token = await recoverPassword({ email, recovery_secret: code });
      setResetToken(token);
      setMode("reset");
      setSuccessMsg("Chave verificada! Crie uma nova senha.");
    } catch (err: any) {
      setError(err.message || "Erro ao recuperar senha");
    } finally {
      setLoading(false);
    }
  }

  async function onResetPassword(data: { new_password: string }) {
    setError("");
    setLoading(true);
    try {
      await resetPassword(resetToken, data.new_password);
      setMode("login");
      setError("");
      setSuccessMsg("");
      setRecoveryFile(null);
      alert("Senha redefinida com sucesso! Faça login com sua nova senha.");
    } catch (err: any) {
      setError(err.message || "Erro ao redefinir senha");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center bg-background">
      <div className="app-surface w-full max-w-sm p-8 space-y-6 animate-fade-in">
        <div className="text-center space-y-1">
          <h1 className="text-2xl font-display font-semibold text-slate-900">AtendeMente</h1>
          <p className="text-sm text-muted-foreground">
            {mode === "login" ? "Faça login para continuar" : mode === "recover" ? "Recuperar Senha" : "Nova Senha"}
          </p>
        </div>

        {error && <p className="text-destructive text-sm bg-destructive/10 p-2 rounded-lg">{error}</p>}
        {successMsg && <p className="text-success text-sm bg-success/10 p-2 rounded-lg">{successMsg}</p>}

        {mode === "recover" && (
          <div className="space-y-4">
            <div className="flex rounded-xl border border-border overflow-hidden">
              <button
                type="button"
                onClick={() => { setRecoverMethod("file"); setError(""); setSuccessMsg(""); }}
                className={`flex-1 py-2 text-sm font-medium transition-colors cursor-pointer ${recoverMethod === "file" ? "bg-primary text-primary-foreground" : "bg-white text-muted-foreground hover:text-foreground"}`}
              >
                Arquivo .json
              </button>
              <button
                type="button"
                onClick={() => { setRecoverMethod("manual"); setError(""); setSuccessMsg(""); }}
                className={`flex-1 py-2 text-sm font-medium transition-colors cursor-pointer ${recoverMethod === "manual" ? "bg-primary text-primary-foreground" : "bg-white text-muted-foreground hover:text-foreground"}`}
              >
                Digitar código
              </button>
            </div>

            {recoverMethod === "file" ? (
              <div className="space-y-4">
                <p className="text-muted-foreground text-sm text-center">
                  Selecione o arquivo de recuperação (.json) que você salvou ao criar a conta.
                </p>
                {recoveryFile ? (
                  <div className="bg-success/10 p-3 rounded-xl border border-success/20">
                    <p className="text-success text-sm font-medium">✓ Arquivo carregado com sucesso!</p>
                    <p className="text-success/70 text-xs mt-1">Usuário: {recoveryFile.user_id.slice(0, 12)}...</p>
                  </div>
                ) : (
                  <button type="button" onClick={handleSelectFile} className="w-full border-2 border-dashed border-border text-muted-foreground py-4 rounded-2xl hover:border-primary hover:text-primary transition-colors">
                    Clique para selecionar o arquivo
                  </button>
                )}
                {recoveryFile && (
                  <button type="button" onClick={handleRecover} disabled={loading} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors">
                    {loading ? "Verificando..." : "Verificar chave"}
                  </button>
                )}
              </div>
            ) : (
              <div className="space-y-4">
                <p className="text-muted-foreground text-sm text-center">
                  Digite seu email e o código de recuperação ({<span className="font-mono text-xs">XXXX-XXXX-XXXX-XXXX</span>}).
                </p>
                <div>
                  <input
                    type="email"
                    placeholder="Email"
                    value={manualEmail}
                    onChange={(e) => setManualEmail(e.target.value)}
                    className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1"
                  />
                </div>
                <div>
                  <input
                    type="text"
                    placeholder="Código de recuperação (XXXX-XXXX-XXXX-XXXX)"
                    value={manualCode}
                    onChange={(e) => setManualCode(e.target.value.toUpperCase())}
                    className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 font-mono tracking-wider"
                  />
                </div>
                <button type="button" onClick={handleManualRecover} disabled={loading} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors">
                  {loading ? "Verificando..." : "Verificar chave"}
                </button>
              </div>
            )}

            <button type="button" onClick={() => { setMode("login"); setError(""); setSuccessMsg(""); setRecoveryFile(null); setManualEmail(""); setManualCode(""); }} className="w-full text-sm text-muted-foreground hover:text-foreground transition-colors">
              Voltar para o login
            </button>
          </div>
        )}

        {mode === "reset" && (
          <form onSubmit={handleResetSubmit(onResetPassword)} className="space-y-4">
            <div>
              <input type="password" placeholder="Nova senha (mínimo 8 caracteres)" {...registerReset("new_password")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
              <FieldError message={resetErrors.new_password?.message} />
            </div>
            <button type="submit" disabled={loading} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors">
              {loading ? "Redefinindo..." : "Redefinir senha"}
            </button>
          </form>
        )}

        {mode === "login" && (
          <form onSubmit={handleSubmit(onLogin)} className="space-y-4">
            <div>
              <input type="email" placeholder="Email" {...register("email")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
              <FieldError message={errors.email?.message} />
            </div>
            <div>
              <input type="password" placeholder="Senha" {...register("password")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
              <FieldError message={errors.password?.message} />
            </div>
            <button type="submit" disabled={loading} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors">
              {loading ? "Entrando..." : "Entrar"}
            </button>
            <div className="flex justify-between text-sm">
              <button type="button" onClick={() => { setMode("recover"); setSuccessMsg(""); }} className="text-primary hover:underline">Esqueceu a senha?</button>
              <Link to="/register" className="text-primary hover:underline">Criar conta</Link>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
