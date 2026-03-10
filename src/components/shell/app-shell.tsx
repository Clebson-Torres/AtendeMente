"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { CalendarDays, CreditCard, House, LogOut, UsersRound } from "lucide-react";
import { signOutAction } from "@/features/auth/actions";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";

const navigation = [
  { href: "/dashboard", label: "Visao geral", icon: House },
  { href: "/agenda", label: "Agenda", icon: CalendarDays },
  { href: "/patients", label: "Pacientes", icon: UsersRound },
  { href: "/financeiro", label: "Financeiro", icon: CreditCard },
];

type AppShellProps = {
  userName: string;
  userEmail: string;
  children: React.ReactNode;
};

type AccountPanelProps = {
  userName: string;
  userEmail: string;
};

function AccountPanel({ userName, userEmail }: AccountPanelProps) {
  return (
    <Card className="border border-border/80 bg-white/90 p-3 shadow-none backdrop-blur">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-semibold text-slate-900">{userName}</p>
          <p className="truncate text-xs text-muted-foreground">{userEmail}</p>
        </div>

        <form action={signOutAction}>
          <Button type="submit" variant="outline" className="border-slate-200 bg-white hover:bg-slate-50">
            <LogOut className="h-4 w-4" />
            Sair
          </Button>
        </form>
      </div>
    </Card>
  );
}

export function AppShell({ userName, userEmail, children }: AppShellProps) {
  const currentPath = usePathname();

  return (
    <div className="min-h-screen">
      <div className="mx-auto flex min-h-screen max-w-[1600px] flex-col gap-4 p-3 sm:p-4 lg:gap-6 lg:p-6">
        <div className="app-surface flex flex-col gap-6 border border-white/80 bg-slate-950 px-5 py-6 text-white lg:px-7 lg:py-7">
          <div className="flex flex-col gap-6 lg:flex-row lg:items-start lg:justify-between">
            <div className="space-y-3">
              <p className="font-display text-5xl leading-none text-white lg:text-6xl">AtendeMente</p>
              <p className="max-w-none whitespace-nowrap text-sm leading-6 text-slate-300 lg:text-base">
                Mais organizacao na rotina, mais foco no atendimento.
              </p>
            </div>

            <div className="w-full lg:max-w-sm">
              <AccountPanel userEmail={userEmail} userName={userName} />
            </div>
          </div>

          <nav className="flex flex-wrap gap-2 lg:gap-3">
            {navigation.map((item) => {
              const Icon = item.icon;
              const isActive = currentPath === item.href || currentPath.startsWith(`${item.href}/`);

              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={`flex min-w-fit items-center gap-3 rounded-2xl px-5 py-4 text-base font-medium transition ${
                    isActive ? "bg-white text-slate-950" : "bg-white/5 text-slate-300 hover:bg-white/10 hover:text-white"
                  }`}
                >
                  <Icon className="h-5 w-5" />
                  {item.label}
                </Link>
              );
            })}
          </nav>

          <div className="hidden">
            <AccountPanel userEmail={userEmail} userName={userName} />
          </div>
        </div>

        <div className="min-w-0 space-y-2">
          <div className="hidden justify-end">
            <div className="w-full max-w-sm">
              <AccountPanel userEmail={userEmail} userName={userName} />
            </div>
          </div>

          <main className="min-w-0 space-y-6 py-0 lg:py-1">{children}</main>
        </div>
      </div>
    </div>
  );
}
