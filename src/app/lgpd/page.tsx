import { LegalDocument } from "@/components/shell/legal-document";

const sections = [
  {
    title: "1. Papel do profissional usuario",
    body: [
      "No contexto do AtendeMente, o psicologo usuario define os dados que insere e utiliza a plataforma como ferramenta de organizacao operacional. Por isso, o uso do produto deve ser acompanhado de criterio profissional e de adequacao ao contexto de cada paciente.",
    ],
  },
  {
    title: "2. Boas praticas recomendadas",
    body: [
      "Utilize apenas os dados necessarios para sua rotina, revise periodicamente o que foi armazenado e mantenha especial cuidado com exportacoes, anexos e acessos por email.",
      "Tambem e recomendado revisar backup, politica de retencao e resposta a incidentes antes de ampliar o numero de pacientes atendidos dentro da plataforma.",
    ],
  },
  {
    title: "3. Medidas do produto",
    body: [
      "O AtendeMente adota autenticacao, RLS, autorizacao por usuario, bucket privado para arquivos, auditoria e criptografia adicional dos registros textuais.",
      "Essas medidas reduzem superficie de risco, mas nao substituem a governanca do profissional sobre seu ambiente de trabalho e seus processos internos.",
    ],
  },
  {
    title: "4. Portabilidade e continuidade",
    body: [
      "A exportacao por paciente existe para apoiar continuidade operacional e portabilidade dos dados quando necessario.",
      "Recomenda-se testar periodicamente esse fluxo e manter registros exportados com as mesmas cautelas de seguranca aplicadas ao ambiente principal.",
    ],
  },
];

export default function LgpdPage() {
  return (
    <LegalDocument
      description="Orientacoes objetivas para uso do AtendeMente com mais cuidado na organizacao e no tratamento de dados no contexto brasileiro."
      eyebrow="LGPD"
      sections={sections}
      title="Cuidados de tratamento de dados no uso do produto"
      updatedAt="10 de marco de 2026"
    />
  );
}
