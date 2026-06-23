import { useNavigate } from "react-router-dom";

export default function NotFound() {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] text-center px-4">
      <h1 className="text-6xl font-bold text-slate-300 mb-4">404</h1>
      <h2 className="text-xl font-semibold text-slate-700 mb-2">Pagina nao encontrada</h2>
      <p className="text-muted-foreground mb-6">
        A pagina que voce procura nao existe ou foi movida.
      </p>
      <button
        onClick={() => navigate("/")}
        className="px-6 py-2 bg-primary text-primary-foreground rounded-xl font-medium hover:opacity-90 transition-opacity"
      >
        Voltar ao Inicio
      </button>
    </div>
  );
}
