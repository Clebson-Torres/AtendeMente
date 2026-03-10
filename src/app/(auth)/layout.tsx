import { SiteFooterLinks } from "@/components/shell/site-footer-links";

export default function AuthLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen">
      {children}
      <div className="px-4 pb-8">
        <SiteFooterLinks className="mx-auto max-w-6xl border-t border-border/60 pt-4" compact />
      </div>
    </div>
  );
}
