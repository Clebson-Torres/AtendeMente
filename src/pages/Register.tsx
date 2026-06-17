import { useState } from "react";
import { useNavigate, Navigate, Link } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { invoke } from "@tauri-apps/api/core";
import { register, completeFromStoredToken } from "../lib/auth";
import { useAuth } from "../App";
import { registerSchema, type RegisterInput } from "../lib/schemas";
import FieldError from "../components/ui/FieldError";
import { Download, ShieldAlert } from "lucide-react";

export default function RegisterPage() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [step, setStep] = useState<"form" | "recovery">("form");
  const [recoverySecret, setRecoverySecret] = useState("");
  const [userId, setUserId] = useState("");

  const { register: reg, handleSubmit, formState: { errors } } = useForm<RegisterInput>({
    resolver: zodResolver(registerSchema),
  });

  if (user) return <Navigate to="/" replace />;

  async function onSubmit(data: RegisterInput) {
    setError("");
    setLoading(true);
    try {
      const result = await register(data.email, data.password, data.full_name);
      setUserId(result.user_id);
      setRecoverySecret(result.recovery_secret);
      setStep("recovery");
    } catch (err: any) {
      setError(err.message || "Erro ao criar conta");
    } finally {
      setLoading(false);
    }
  }

  async function downloadRecoveryFile() {
    try {
      const content = JSON.stringify({ version: 1, user_id: userId, recovery_secret: recoverySecret }, null, 2);
      const filename = `atendemente-recovery-${userId.slice(0, 8)}.json`;
      await invoke("save_recovery_file", { filename, content });
    } catch (err: any) {
      if (err !== "Salvamento cancelado.") setError(err?.toString() || "Erro ao salvar arquivo");
    }
  }

  async function finish() {
    await completeFromStoredToken();
    navigate("/");
  }

  if (step === "recovery") {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="app-surface w-full max-w-md p-8 space-y-6 animate-fade-in">
          <h1 className="text-2xl font-display font-semibold text-slate-900 text-center">Chave de Recuperação</h1>
          <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-xl">
            <div className="flex gap-3">
              <ShieldAlert className="h-5 w-5 text-yellow-600 flex-shrink-0 mt-0.5" />
              <div>
                <p className="text-yellow-800 font-medium">Salve esta chave em um local seguro!</p>
                <p className="text-yellow-700 text-sm mt-1">Sem este arquivo, <strong>não será possível recuperar sua senha</strong> caso você a esqueça. A chave é única e não pode ser regenerada.</p>
              </div>
            </div>
          </div>
          <div className="bg-muted p-4 rounded-2xl border border-border">
            <p className="text-sm font-mono text-muted-foreground break-all bg-white p-3 rounded-xl border border-border">{recoverySecret}</p>
          </div>
          <button onClick={downloadRecoveryFile} className="w-full flex items-center justify-center gap-2 bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium transition-colors">
            <Download className="h-4 w-4" /> Baixar arquivo de recuperação
          </button>
          <button onClick={finish} className="w-full bg-success text-success-foreground py-2.5 rounded-xl hover:bg-success/90 font-medium transition-colors">
            Já salvei. Ir para o sistema!
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen items-center justify-center bg-background">
      <div className="app-surface w-full max-w-sm p-8 space-y-6 animate-fade-in">
        <h1 className="text-2xl font-display font-semibold text-slate-900 text-center">AtendeMente</h1>
        <p className="text-muted-foreground text-center text-sm">Crie sua conta</p>
        {error && <p className="text-destructive text-sm bg-destructive/10 p-2 rounded-lg">{error}</p>}
        <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
          <div>
            <input type="text" placeholder="Nome completo" {...reg("full_name")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
            <FieldError message={errors.full_name?.message} />
          </div>
          <div>
            <input type="email" placeholder="Email" {...reg("email")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
            <FieldError message={errors.email?.message} />
          </div>
          <div>
            <input type="password" placeholder="Senha (mínimo 8 caracteres)" {...reg("password")} className="flex h-10 w-full rounded-2xl border border-input bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1" />
            <FieldError message={errors.password?.message} />
          </div>
          <button type="submit" disabled={loading} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium disabled:opacity-50 transition-colors">
            {loading ? "Criando conta..." : "Criar conta"}
          </button>
          <p className="text-center text-sm text-muted-foreground">Já tem conta? <Link to="/login" className="text-primary hover:underline">Fazer login</Link></p>
        </form>
      </div>
    </div>
  );
}
