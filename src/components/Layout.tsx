import { NavLink } from "react-router-dom";
import { useAuth } from "../App";
import { logout } from "../lib/auth";
import { House, CalendarDays, UsersRound, CreditCard, LogOut } from "lucide-react";
import { cn } from "../lib/utils";

const nav = [
  { label: "Visão geral", href: "/", icon: House },
  { label: "Agenda", href: "/appointments", icon: CalendarDays },
  { label: "Pacientes", href: "/patients", icon: UsersRound },
  { label: "Financeiro", href: "/payments", icon: CreditCard },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();

  return (
    <div className="flex h-screen bg-background">
      <aside className="w-64 bg-slate-900 flex flex-col">
        <div className="p-5 border-b border-slate-700/50">
          <h1 className="font-display text-2xl font-semibold text-white">
            AtendeMente
          </h1>
          <p className="text-xs text-slate-400 mt-0.5">Gestão de Clínica</p>
        </div>
        <nav className="flex-1 p-3 space-y-1">
          {nav.map(({ label, href, icon: Icon }) => (
            <NavLink
              key={href}
              to={href}
              end={href === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-colors",
                  isActive
                    ? "bg-white/10 text-white"
                    : "text-slate-300 hover:text-white hover:bg-white/5",
                )
              }
            >
              <Icon className="h-4 w-4" />
              {label}
            </NavLink>
          ))}
        </nav>
        <div className="p-4 border-t border-slate-700/50">
          <p className="text-sm text-slate-400 truncate">{user?.email}</p>
          <button
            onClick={logout}
            className="mt-2 flex items-center gap-2 text-sm text-slate-400 hover:text-red-400 transition-colors"
          >
            <LogOut className="h-3.5 w-3.5" />
            Sair
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-auto">{children}</main>
    </div>
  );
}
