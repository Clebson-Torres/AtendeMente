import { useState } from "react";
import { useNavigate, Navigate, Link } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { register } from "../lib/auth";
import { useAuth } from "../App";
import { registerSchema, type RegisterInput } from "../lib/schemas";
import FieldError from "../components/ui/FieldError";

export default function RegisterPage() {
  const navigate = useNavigate();
  const { user } = useAuth();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const { register: reg, handleSubmit, formState: { errors } } = useForm<RegisterInput>({
    resolver: zodResolver(registerSchema),
  });

  if (user) return <Navigate to="/" replace />;

  async function onSubmit(data: RegisterInput) {
    setError("");
    setLoading(true);
    try {
      await register(data.email, data.password, data.full_name);
      navigate("/onboarding");
    } catch (err: any) {
      setError(err.message || "Erro ao criar conta");
    } finally {
      setLoading(false);
    }
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
