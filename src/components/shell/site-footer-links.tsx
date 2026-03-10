import Link from "next/link";

type SiteFooterLinksProps = {
  className?: string;
  compact?: boolean;
};

export function SiteFooterLinks({ className, compact = false }: SiteFooterLinksProps) {
  return (
    <div className={className}>
      <div
        className={`flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-muted-foreground ${
          compact ? "justify-center" : "justify-between"
        }`}
      >
        <div className="flex flex-wrap items-center gap-4">
          <Link className="transition hover:text-foreground" href="/privacidade">
            Privacidade
          </Link>
          <Link className="transition hover:text-foreground" href="/termos">
            Termos
          </Link>
          <Link className="transition hover:text-foreground" href="/lgpd">
            LGPD
          </Link>
        </div>
        <p>AtendeMente. Operacao orientada por convite e importacao oficial via CSV.</p>
      </div>
    </div>
  );
}
