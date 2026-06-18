import { useState } from "react";
import { unlock } from "../lib/auth";
import Input from "./ui/Input";
import Button from "./ui/Button";
import { Lock } from "lucide-react";

interface Props {
  onUnlock: () => void;
}

export default function LockScreen({ onUnlock }: Props) {
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await unlock(password);
      onUnlock();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ backgroundColor: "hsl(var(--primary))" }}>
      <form onSubmit={handleUnlock} className="bg-white rounded-3xl shadow-2xl p-8 w-80 space-y-5 app-surface">
        <div className="text-center space-y-2">
          <div className="mx-auto h-12 w-12 rounded-full bg-primary/10 flex items-center justify-center">
            <Lock className="h-6 w-6 text-primary" />
          </div>
          <h1 className="text-xl font-display font-semibold text-slate-900">Tela Bloqueada</h1>
          <p className="text-sm text-muted-foreground">Digite sua senha para desbloquear</p>
        </div>
        <Input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="Senha"
          autoFocus
        />
        {error && <p className="text-destructive text-sm">{error}</p>}
        <Button type="submit" disabled={loading || !password} className="w-full">
          {loading ? "Desbloqueando..." : "Desbloquear"}
        </Button>
      </form>
    </div>
  );
}
