import { useState } from "react";
import { useNavigate, Navigate, Link } from "react-router-dom";
import { login, recoverPassword, resetPassword } from "../lib/auth";
import { useAuth } from "../App";

export default function Login() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<"login" | "recover" | "reset">("login");
  const [recoveryFile, setRecoveryFile] = useState<{ user_id: string; recovery_secret: string } | null>(null);
  const [resetToken, setResetToken] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [successMsg, setSuccessMsg] = useState("");
  const navigate = useNavigate();
  const { user } = useAuth();

  if (user) return <Navigate to="/" replace />;

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await login(email, password);
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
        if (!data.user_id || !data.recovery_secret) {
          throw new Error("Arquivo inválido");
        }
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
    setLoading(true);
    try {
      const token = await recoverPassword(recoveryFile.user_id, recoveryFile.recovery_secret);
      setResetToken(token);
      setMode("reset");
      setSuccessMsg("Chave verificada! Crie uma nova senha.");
    } catch (err: any) {
      setError(err.message || "Erro ao recuperar senha");
    } finally {
      setLoading(false);
    }
  }

  async function handleResetPassword(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await resetPassword(resetToken, newPassword);
      setSuccessMsg("");
      setMode("login");
      setError("");
      setNewPassword("");
      setRecoveryFile(null);
      setEmail("");
      setPassword("");
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
            <p className="text-muted-foreground text-sm text-center">
              Selecione o arquivo de recuperação (.json) que você salvou ao criar a conta.
            </p>
            {recoveryFile ? (
              <div className="bg-success/10 p-3 rounded-xl border border-success/20">
                <p className="text-success text-sm font-medium">✓ Arquivo carregado com sucesso!</p>
                <p className="text-success/70 text-xs mt-1">Usuário: {recoveryFile.user_id.slice(0, 12)}...</p>
              </div>
            ) : (
              <button
                onClick={handleSelectFile}
                className="w-full border-2 border-dashed border-border text-muted-foreground py-4 rounded-2xl hover:border-primary hover:text-primary transition-colors"
              >
                Clique para selecionar o arquivo
              </button>
            )}
            {recoveryFile && (
              <button
                onClick={handleRecover}
                disabled={loading}
                className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors"
              >
                {loading ? "Verificando..." : "Verificar chave"}
              </button>
            )}
            <button
              onClick={() => { setMode("login"); setError(""); setRecoveryFile(null); }}
              className="w-full text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              Voltar para o login
            </button>
          </div>
        )}

        {mode === "reset" && (
          <form onSubmit={handleResetPassword} className="space-y-4">
            <input
              type="password"
              placeholder="Nova senha (mínimo 6 caracteres)"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              required
              minLength={6}
              className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1"
            />
            <button
              type="submit"
              disabled={loading}
              className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors"
            >
              {loading ? "Redefinindo..." : "Redefinir senha"}
            </button>
          </form>
        )}

        {mode === "login" && (
          <form onSubmit={handleLogin} className="space-y-4">
            <input
              type="email"
              placeholder="Email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1"
            />
            <input
              type="password"
              placeholder="Senha"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1"
            />
            <button
              type="submit"
              disabled={loading}
              className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors"
            >
              {loading ? "Entrando..." : "Entrar"}
            </button>
            <div className="flex justify-between text-sm">
              <button
                type="button"
                onClick={() => setMode("recover")}
                className="text-primary hover:underline"
              >
                Esqueceu a senha?
              </button>
              <Link to="/register" className="text-primary hover:underline">
                Criar conta
              </Link>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
