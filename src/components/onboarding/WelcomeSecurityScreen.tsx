import { Lock, Shield, ShieldAlert } from "lucide-react";

interface Props {
  onNext: () => void;
}

export default function WelcomeSecurityScreen({ onNext }: Props) {
  return (
    <div className="app-surface w-full max-w-md p-8 space-y-6 animate-fade-in">
      <div className="text-center">
        <div className="mx-auto h-14 w-14 rounded-2xl bg-primary/10 flex items-center justify-center mb-4">
          <Lock className="h-7 w-7 text-primary" />
        </div>
        <h1 className="text-2xl font-display font-semibold text-slate-900">Bem-vindo ao AtendeMente</h1>
        <p className="text-muted-foreground text-sm mt-2">Configure a proteção dos seus dados em apenas alguns passos.</p>
      </div>

      <div className="space-y-3">
        <div className="bg-cyan-50 border-l-4 border-cyan-400 p-4 rounded-xl">
          <div className="flex gap-3">
            <Shield className="h-5 w-5 text-cyan-600 flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-cyan-800 font-medium">Seus dados são criptografados</p>
              <p className="text-cyan-700 text-sm mt-1">Todas as informações sensíveis dos seus pacientes são protegidas com criptografia de ponta a ponta antes de serem armazenadas.</p>
            </div>
          </div>
        </div>

        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-xl">
          <div className="flex gap-3">
            <ShieldAlert className="h-5 w-5 text-yellow-600 flex-shrink-0 mt-0.5" />
            <div>
              <p className="text-yellow-800 font-medium">Nunca compartilhe sua senha ou código de recuperação</p>
              <p className="text-yellow-700 text-sm mt-1">O AtendeMente nunca pedirá sua senha ou código de recuperação por email, telefone ou mensagem.</p>
            </div>
          </div>
        </div>
      </div>

      <button onClick={onNext} className="w-full bg-primary text-primary-foreground py-2.5 rounded-xl hover:bg-primary/90 font-medium transition-colors cursor-pointer">
        Continuar
      </button>
    </div>
  );
}
