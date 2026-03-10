import { LegalDocument } from "@/components/shell/legal-document";

const sections = [
  {
    title: "1. O que o AtendeMente faz",
    body: [
      "O AtendeMente e uma plataforma para psicologos organizarem agenda, pacientes, pagamentos manuais, registros de atendimento, anexos privados e exportacao dos dados por paciente.",
      "Esta pagina apresenta, em linguagem objetiva, como os dados sao tratados no contexto do produto e quais cuidados operacionais o profissional precisa manter no uso da plataforma.",
    ],
  },
  {
    title: "2. Dados tratados",
    body: [
      "O produto pode armazenar dados administrativos do profissional, dados cadastrais de pacientes, agenda de sessoes, status financeiros, registros textuais de atendimento e arquivos vinculados aos atendimentos.",
      "O profissional usuario e responsavel por inserir apenas os dados necessarios para sua rotina e por manter a informacao atualizada e adequada ao contexto clinico e administrativo.",
    ],
  },
  {
    title: "3. Como os dados sao protegidos",
    body: [
      "O AtendeMente utiliza autenticacao, isolamento por usuario, controles de autorizacao no backend, bucket privado para anexos, trilha de auditoria e criptografia adicional dos registros textuais de atendimento.",
      "Apesar dessas medidas, o uso seguro tambem depende da configuracao correta do ambiente publicado, da protecao das credenciais do profissional e do uso responsavel do dispositivo onde a conta e acessada.",
    ],
  },
  {
    title: "4. Compartilhamento e acesso",
    body: [
      "Os dados nao sao expostos publicamente pelo produto. O acesso depende de autenticacao e, no caso de anexos e exportacoes, tambem passa por validacao no backend.",
      "O profissional usuario deve revisar cuidadosamente quem tem acesso a sua conta, seus dispositivos e seus canais de email, especialmente para convites, redefinicao de senha e exportacao de dados.",
    ],
  },
  {
    title: "5. Retencao e exclusao",
    body: [
      "O produto utiliza exclusao logica em pontos importantes para preservar historico operacional e auditoria. Isso ajuda a evitar perdas acidentais e facilita rastreabilidade.",
      "Quando houver necessidade de exclusao definitiva ou encerramento de uso, a politica operacional deve ser avaliada em conjunto com a rotina do consultorio e com as obrigacoes aplicaveis ao profissional.",
    ],
  },
];

export default function PrivacyPage() {
  return (
    <LegalDocument
      description="Resumo do tratamento de dados e das medidas de protecao aplicadas no AtendeMente para apoiar um uso profissional e responsavel."
      eyebrow="Politica de privacidade"
      sections={sections}
      title="Privacidade e tratamento de dados"
      updatedAt="10 de marco de 2026"
    />
  );
}
