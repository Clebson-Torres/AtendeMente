import { LegalDocument } from "@/components/shell/legal-document";

const sections = [
  {
    title: "1. Uso profissional",
    body: [
      "O AtendeMente foi desenhado para apoiar a organizacao da rotina de psicologos autonomos. O uso do produto deve respeitar a finalidade profissional da plataforma e a responsabilidade do usuario sobre os dados inseridos.",
    ],
  },
  {
    title: "2. Credenciais e acesso",
    body: [
      "Cada conta e individual e depende de convite controlado. O usuario deve manter senha forte, proteger seus dispositivos e revisar periodicamente os acessos autorizados.",
      "Nao e recomendado compartilhar credenciais ou manter a conta aberta em dispositivos de uso coletivo sem controles adequados.",
    ],
  },
  {
    title: "3. Uso adequado da plataforma",
    body: [
      "O usuario deve utilizar o produto dentro do escopo do MVP atual: agenda, cadastro de pacientes, pagamentos manuais, registros leves, anexos privados e exportacao por paciente.",
      "O uso de arquivos maliciosos, tentativas de burlar autorizacao, automacao abusiva ou qualquer acao que comprometa a seguranca do ambiente pode resultar em bloqueio operacional do acesso.",
    ],
  },
  {
    title: "4. Disponibilidade e suporte",
    body: [
      "Por se tratar de um produto em evolucao, algumas rotinas podem depender de operacao controlada, como convites, revisao de configuracao ou apoio em migracao de base.",
      "Antes de uso comercial amplo, o profissional deve validar seu fluxo de backups, notificacoes operacionais e testes de recuperacao de acesso.",
    ],
  },
  {
    title: "5. Exportacao e responsabilidade do usuario",
    body: [
      "O produto oferece exportacao estruturada por paciente para facilitar continuidade operacional, backup e portabilidade.",
      "Cabe ao usuario armazenar com seguranca os arquivos exportados e evitar compartilhamento indevido dos dados fora do contexto autorizado.",
    ],
  },
];

export default function TermsPage() {
  return (
    <LegalDocument
      description="Condicoes gerais de uso do AtendeMente para uma operacao profissional, segura e alinhada ao escopo atual do produto."
      eyebrow="Termos de uso"
      sections={sections}
      title="Regras de uso da plataforma"
      updatedAt="10 de marco de 2026"
    />
  );
}
