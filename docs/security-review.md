# Relatorio de Seguranca - AtendeMente MVP

## Resumo executivo

O AtendeMente esta em um patamar consistente para venda controlada: autenticacao via Supabase Auth, isolamento por `user_id` com RLS, bucket privado, downloads autorizados por backend, criptografia `AES-256-GCM` para registros textuais, auditoria de eventos criticos, headers HTTP de seguranca, rate limiting nas rotas sensiveis principais e validacao reforcada de upload no backend.

Com a remocao do suporte a `.xls/.xlsx`, o retorno para `CSV` como formato oficial de importacao e o hardening recente do fluxo de upload, o maior risco imediato de supply chain foi reduzido. O produto ainda nao deve ser tratado como endurecimento maximo de producao, mas agora esta mais proximo de uma venda controlada com operacao guiada.

## Controles existentes

### Autenticacao e sessao

- `Supabase Auth` com login por email e senha.
- onboarding por convite controlado.
- reset de senha com fluxo nativo do Supabase e telas dedicadas do app.
- middleware protegendo rotas autenticadas.

### Autorizacao e isolamento de dados

- `user_id` presente nas entidades sensiveis.
- `RLS` em tabelas de dados do produto.
- filtros explicitos por usuario nas queries e services.
- triggers de ownership para validar coerencia entre entidades relacionadas.

### Arquivos e storage

- bucket privado no Supabase Storage.
- upload iniciado por backend com validacao e URL assinada.
- validacao de extensao permitida antes do signed upload.
- confirmacao do upload baseada no objeto real do bucket, com verificacao de tipo e tamanho.
- download apenas via backend autenticado.

### Criptografia

- `session_records` criptografados com `AES-256-GCM`.
- chave mantida no backend via ambiente.
- service role key nao exposta ao frontend.

### Auditoria

- eventos relevantes auditados:
  - login
  - logout
  - upload
  - download
  - exportacao
  - updates importantes

### Hardening HTTP

- headers configurados:
  - `X-Frame-Options`
  - `X-Content-Type-Options`
  - `Referrer-Policy`
  - `Permissions-Policy`
  - `Content-Security-Policy`
- `unsafe-eval` removido em producao.
- `object-src 'none'` aplicado.

### Rate limiting

- limites aplicados em:
  - login
  - reset de senha
  - upload
  - importacao
  - exportacao
- falha de infraestrutura de rate limit agora fecha o fluxo com erro controlado, em vez de desligar a protecao silenciosamente.

## Achados e riscos priorizados

### Media severidade

#### 1. Rate limiting ainda e local ao banco da aplicacao

- Evidencia:
  - os limites agora existem e falham de forma fechada se a tabela operacional estiver ausente, mas continuam dependentes do banco principal e nao de uma camada dedicada de edge/KV.
- Impacto:
  - suficiente para venda controlada, mas nao ideal para carga maior ou protecao distribuida.
- Tipo:
  - risco estrutural / operacional.
- Recomendacao:
  - medio prazo: mover limites para camada mais apropriada de edge/KV ou provedor gerenciado.

#### 2. CSP ainda pode ser refinada

- Evidencia:
  - `unsafe-eval` saiu de producao, mas `unsafe-inline` ainda permanece por compatibilidade com o app atual.
- Impacto:
  - baseline melhor, mas ainda existe margem para endurecimento adicional.
- Tipo:
  - risco operacional/configuracao.
- Recomendacao:
  - revisar CSP em ambiente real e migrar gradualmente para nonce/hash quando viavel.

#### 3. Convites dependem de operacao controlada

- Evidencia:
  - onboarding continua restrito a convites enviados com service role.
- Impacto:
  - positivo para seguranca, mas requer processo operacional claro para nao virar gargalo.
- Tipo:
  - risco operacional.
- Recomendacao:
  - documentar o procedimento de convite, expiracao e reenvio.

### Baixa severidade

#### 4. Mensagens externas ainda podem ser mais neutras em alguns fluxos

- Evidencia:
  - parte das rotas e actions ainda responde com mensagens funcionais relativamente especificas.
- Impacto:
  - baixo para o momento, mas vale reduzir pistas em ambiente comercial.
- Tipo:
  - risco operacional.
- Recomendacao:
  - padronizar mensagens externas e concentrar detalhe tecnico em logs internos.

#### 5. Auditoria de IP depende de proxy confiavel

- Evidencia:
  - o app usa `x-forwarded-for` quando disponivel.
- Impacto:
  - baixo, mas precisa de configuracao correta em producao.
- Tipo:
  - risco operacional/configuracao.
- Recomendacao:
  - documentar proxy confiavel na Vercel e revisar logs reais.

## Risco reduzido nesta rodada

### Remocao de `.xls/.xlsx`

- O suporte a `.xls/.xlsx` foi removido do fluxo comercial.
- A importacao oficial agora usa apenas `CSV` exportado do Excel.
- Isso reduz o risco associado a parsing de planilhas binarias e elimina a dependencia `xlsx` do projeto.

### Validacao reforcada de upload

- O backend agora valida extensao permitida antes do signed upload.
- Na confirmacao, o arquivo real no bucket e conferido por tipo e tamanho antes de ser aceito.
- Upload adulterado e removido do storage e o registro correspondente e invalidado.

## Pontos fortes do estado atual

- Isolamento por usuario esta bem modelado em banco e aplicacao.
- Criptografia adicional protege registros textuais em repouso.
- Download privado continua mediado pelo backend.
- Exportacao de paciente respeita autorizacao e gera auditoria.
- Onboarding por convite reduz exposicao operacional em comparacao com signup publico.
- O build de release passa com limpeza de artefatos do Next antes da geracao da versao final.

## Recomendacoes priorizadas

### Imediato

1. Manter onboarding apenas por convite enquanto o produto amadurece.
2. Aplicar todas as migrations novas no Supabase antes de comercializar.
3. Revisar email templates do Supabase para convite e reset de senha.
4. Publicar e revisar os documentos legais minimos (`/privacidade`, `/termos`, `/lgpd`).

### Curto prazo

1. Refinar CSP com base no ambiente real de producao.
2. Padronizar mensagens externas mais neutras em todas as rotas sensiveis.
3. Criar rotina operacional de convites e recuperacao de conta.
4. Formalizar checklist de lancamento e rotina de backup/restauracao.

### Medio prazo

1. Evoluir rate limiting para camada mais apropriada para escala.
2. Avaliar 2FA para contas com maior sensibilidade.
3. Adicionar monitoramento de anomalias de login, exportacao e upload.

## Riscos aceitos / pendencias

- O relatorio continua sendo uma analise tecnica do codigo e da configuracao local, nao um pentest externo.
- A seguranca final depende tambem da configuracao correta do Supabase, da Vercel e dos emails transacionais.
- O modelo de banco continua sendo compartilhado, com isolamento por `RLS` e `user_id`, o que e adequado para SaaS moderno quando bem implementado.

## Nota de prontidao comercial

- Antes desta rodada: `62/100`
- Depois desta rodada, com as medidas aplicadas no codigo: estimativa de `82/100` para venda controlada

### O que ainda falta para 85+

- validar convite e reset de senha ponta a ponta no Supabase web
- reforcar e2e autenticado para os fluxos principais
- revisar templates e entregabilidade de email
- endurecer mais a operacao de producao
