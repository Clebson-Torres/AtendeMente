import Link from "next/link";

type LegalDocumentSection = {
  title: string;
  body: string[];
};

type LegalDocumentProps = {
  eyebrow: string;
  title: string;
  description: string;
  updatedAt: string;
  sections: LegalDocumentSection[];
};

export function LegalDocument({ eyebrow, title, description, updatedAt, sections }: LegalDocumentProps) {
  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto max-w-4xl space-y-8 px-4 py-10 sm:px-6 lg:px-8">
        <div className="space-y-4 rounded-[32px] border border-border/80 bg-white p-8 shadow-sm">
          <div className="space-y-2">
            <p className="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">{eyebrow}</p>
            <h1 className="font-display text-4xl text-slate-950 sm:text-5xl">{title}</h1>
            <p className="max-w-3xl text-base leading-7 text-muted-foreground">{description}</p>
          </div>

          <div className="flex flex-wrap items-center gap-4 text-sm text-muted-foreground">
            <span>Atualizado em {updatedAt}</span>
            <Link className="font-medium text-primary hover:underline" href="/login">
              Voltar para o acesso
            </Link>
          </div>
        </div>

        <div className="space-y-5 rounded-[32px] border border-border/80 bg-white p-8 shadow-sm">
          {sections.map((section) => (
            <section key={section.title} className="space-y-3">
              <h2 className="text-xl font-semibold text-slate-950">{section.title}</h2>
              <div className="space-y-3 text-sm leading-7 text-slate-700 sm:text-base">
                {section.body.map((paragraph) => (
                  <p key={paragraph}>{paragraph}</p>
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
