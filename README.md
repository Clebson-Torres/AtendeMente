# AtendeMente

AtendeMente e um MVP web para psicologos autonomos no Brasil organizarem agenda, pacientes, pagamentos manuais, registros de atendimento, anexos privados e exportacao completa por paciente.

## Stack

- Next.js 15 com App Router
- TypeScript
- Tailwind CSS + componentes no estilo `shadcn/ui`
- Supabase Auth + Storage
- Drizzle ORM + PostgreSQL
- React Hook Form + Zod
- FullCalendar
- Vitest + Playwright

## Direcao do MVP

- `login/logout` com onboarding por convite
- isolamento completo por `user_id`
- bucket privado para anexos
- criptografia adicional dos registros textuais com `AES-256-GCM`
- exportacao sincronica em `ZIP` com `manifest.json` e anexos
- `soft delete` como politica padrao do produto

## Estrutura

```text
src/
  app/                 rotas, layouts e route handlers
  components/          UI reutilizavel, shell e formularios
  features/            regras e servicos por dominio
  db/                  schema Drizzle e cliente de banco
  lib/                 auth, supabase, crypto, audit, utils
supabase/
  migrations/          SQL de schema, RLS, triggers e bucket privado
  config.toml          configuracao local do Supabase CLI
scripts/
  seed-demo.ts         seed opcional com usuario demo
tests/
  unit/                validacoes e utilitarios
  integration/         contratos e composicao de dominios
  e2e/                 smoke tests com Playwright
```

## Variaveis de ambiente

Copie `.env.example` para `.env.local` e preencha:

```bash
NEXT_PUBLIC_SUPABASE_URL=http://127.0.0.1:54321
NEXT_PUBLIC_SUPABASE_ANON_KEY=...
NEXT_PUBLIC_APP_URL=http://localhost:3000
SUPABASE_SERVICE_ROLE_KEY=...
DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:54322/postgres
APP_ENCRYPTION_KEY=base64-encoded-32-byte-key
PRIVATE_STORAGE_BUCKET=private-record-files
E2E_EMAIL=demo@atendemente.local
E2E_PASSWORD=AtendeMente123!
```

Para gerar uma chave valida:

```bash
node -e "console.log(require('node:crypto').randomBytes(32).toString('base64'))"
```

## Como rodar localmente

1. Instale dependencias:

```bash
npm install
```

2. Instale o Supabase CLI se ainda nao tiver:

```bash
npm install -g supabase
```

3. Suba a stack local do Supabase:

```bash
supabase start
```

4. Copie `.env.example` para `.env.local` e ajuste as chaves locais exibidas pelo CLI.

5. Reaplique banco, RLS e bucket:

```bash
supabase db reset
```

6. Gere dados de demonstração opcionais:

```bash
npm run seed:demo
```

O seed cria:

- usuario demo: `demo@atendemente.local`
- senha demo: `AtendeMente123!`

7. Rode a aplicacao:

```bash
npm run dev
```

Abra [http://localhost:3000/login](http://localhost:3000/login).

## Scripts

```bash
npm run dev
npm run build
npm run lint
npm run typecheck
npm run test
npm run test:unit
npm run test:integration
npm run test:e2e
npm run verify:release
npm run db:reset
npm run seed:demo
npm run invite:user -- email@dominio.com "Nome Opcional"
```

## Release gate

Antes de qualquer deploy, execute:

```bash
npm run verify:release
```

O build agora limpa `.next` antes de gerar a versao de producao, reduzindo risco de artefato stale entre ambientes.

Para os e2e autenticados de negocio, configure em `.env.local`:

```bash
E2E_EMAIL=demo@atendemente.local
E2E_PASSWORD=AtendeMente123!
```

Se estiver usando Supabase local, rode `npm run seed:demo` antes da suite ponta a ponta.

## Seguranca implementada

- RLS em todas as tabelas sensiveis
- validacao explicita por `user_id` nos servicos do backend
- uploads privados com `signed upload URL`
- download apenas via backend autenticado
- `session_records` criptografados antes da persistencia
- logs de auditoria para login, logout, upload, download, exportacao e updates relevantes
- rate limiting em login, reset de senha, upload, importacao e exportacao
- headers de seguranca HTTP no app

## Onboarding de usuarios

- novos usuarios entram por convite controlado
- o convite e enviado pelo Supabase Auth
- fluxo de reset de senha usa o proprio Supabase, com telas do app
- para convidar um novo usuario:

```bash
npm run invite:user -- email@dominio.com "Nome Opcional"
```

## Escopo intencionalmente fora do MVP

- signup publico aberto
- pagamentos online
- portal do paciente
- prontuario clinico complexo
- equipe multiusuario por consultorio

## Observacoes importantes

- O app usa Drizzle para schema e queries, mas as migrations SQL vivem em `supabase/migrations` para integrar melhor com Supabase local.
- O banco local pode ser acessado diretamente via `DATABASE_URL`; por isso os servicos sempre filtram por `actorUserId`, mesmo com RLS ativo.
- O bucket privado e criado pela migration inicial com limite de 10 MB e MIME types controlados.
- O formato oficial de importacao comercial e `CSV` exportado do Excel.
- Os documentos publicos minimos ficam em:
  - `/privacidade`
  - `/termos`
  - `/lgpd`
- A checklist operacional de lancamento esta em `docs/launch-checklist.md`.
