import { AppShell } from "@/components/shell/app-shell";
import { getCurrentAppUser } from "@/lib/auth/session";
import { getRequestCspNonce } from "@/lib/security/csp";

export default async function ProtectedLayout({ children }: { children: React.ReactNode }) {
  await getRequestCspNonce();
  const user = await getCurrentAppUser();

  return (
    <AppShell userName={user.fullName} userEmail={user.email ?? "Sem email"}>
      {children}
    </AppShell>
  );
}
