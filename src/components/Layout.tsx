import { NavLink } from "react-router-dom";
import { useAuth } from "../App";
import { logout } from "../lib/auth";

const nav = [
  { label: "Dashboard", href: "/" },
  { label: "Pacientes", href: "/patients" },
  { label: "Agenda", href: "/appointments" },
  { label: "Financeiro", href: "/payments" },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();

  return (
    <div className="flex h-screen bg-gray-50">
      <aside className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-4 border-b border-gray-200">
          <h1 className="text-xl font-bold text-blue-600">AtendeMente</h1>
        </div>
        <nav className="flex-1 p-2 space-y-1">
          {nav.map((item) => (
            <NavLink
              key={item.href}
              to={item.href}
              end={item.href === "/"}
              className={({ isActive }) =>
                `block px-3 py-2 rounded-lg transition-colors ${
                  isActive
                    ? "bg-blue-50 text-blue-600 font-medium"
                    : "text-gray-700 hover:bg-blue-50 hover:text-blue-600"
                }`
              }
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="p-4 border-t border-gray-200">
          <p className="text-sm text-gray-500 truncate">{user?.email}</p>
          <button
            onClick={logout}
            className="mt-2 text-sm text-red-500 hover:text-red-700"
          >
            Sair
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-auto">{children}</main>
    </div>
  );
}
