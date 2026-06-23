# AtendeMente

<p align="center">
  <img src="src-tauri/icons/icon3.png" width="128" height="128" alt="AtendeMente Logo" style="border-radius: 24px; background: #1e293b; padding: 12px;">
</p>

<p align="center">
  <a href="https://clebson-torres.github.io/atendemente.app/">clebson-torres.github.io/atendemente.app</a>
</p>

Aplicação desktop para gestão de consultórios de psicologia. Combina shell Tauri v2,
frontend React/Vite/TypeScript com API embarcada Rust/Axum e SQLite local.

Mais detalhes sobre o projeto em [clebson-torres.github.io/atendemente.app](https://clebson-torres.github.io/atendemente.app/).

## Stack

| Camada     | Tecnologia                                              |
|------------|---------------------------------------------------------|
| Desktop    | Tauri v2                                                |
| Frontend   | React 18, TypeScript, Vite 6, Tailwind CSS 3, Zod 4    |
| Backend    | Rust, Axum, Tokio, SQLx                                 |
| Banco      | SQLite (1 por usuário)                                  |
| Hash senha | Argon2id (19 MiB, 2 iterações)                          |
| Criptografia | AES-256-GCM + HKDF-SHA256 (por usuário)              |
| Keychain   | keyring v3 (Windows Credential Manager / macOS Keychain)|
| Testes     | Vitest (unit), Playwright (E2E) — 96 frontend, 86 Rust |

## Funcionalidades

- **Autenticação local** — email + senha com Argon2id, rate limit (5 tentativas/10min)
- **Recuperação de conta** — secret recovery de 32 bytes (hash SHA-256), formato `XXXX-XXXX-XXXX-XXXX`
- **Recuperação manual** — digitar código de recuperação manualmente (fallback sem arquivo `.json`)
- **Bloqueio por inatividade** — overlay após 5 min, exige senha para desbloquear
- **Onboarding** — 3 telas (boas-vindas, secret recovery, backup) para novos registros
- **Status de segurança** — card no Dashboard com visão geral da proteção da conta
- **Pacientes (CRUD criptografado)** — nome, telefone, email, data nascimento, histórico clínico,
  medicações, anotações — tudo AES-256-GCM, chave derivada por usuário via HKDF
- **WhatsApp** — link direto `wa.me` no detalhe do paciente
- **Google Meet** — botão "Iniciar Atendimento" no detalhe do paciente
- **Busca indexada** — nome (plaintext) + tokens de busca para telefone/email (índice `patient_search_tokens`)
- **Detecção de duplicatas** — identidade por nome + telefone via token `identity_key`
- **Agendamento** — consultas com status, duração, observações, reagendamento
- **Recorrência** — suporte a consultas recorrentes (semanal, quinzenal, mensal, etc.)
- **Cancelar série** — opção de cancelar toda a série recorrente no detalhe do agendamento
- **Pagamentos** — registro de valores, métodos, status (pendente/pago/cancelado)
- **Dashboard** — cards com totais e gráfico de agendamentos dos próximos dias
- **Timeline do paciente** — linha do tempo com consultas e pagamentos consolidados
- **Exportação** — dados do paciente em formato ZIP, CSV de pacientes/agenda/financeiro
- **Upload de arquivos** — anexos por consulta (armazenamento local criptografado)
- **Backup criptografado** — exportação `.atendemente` com AES-256-GCM + senha opcional
- **Acesso Mobile** — toggle em Configurações (desativado por padrão), QR code quando ativo
- **Headers de segurança** — CSP restritivo, X-Frame-Options DENY, X-Content-Type-Options nosniff
- **Auditoria** — logs de acesso e alterações sensíveis

## Screenshots

<p align="center">
  <img src="screenshots/visaogeral.jpeg" width="45%" alt="Visão Geral" />
  <img src="screenshots/agenda.jpeg" width="45%" alt="Agenda" />
</p>
<p align="center">
  <img src="screenshots/pacientes.jpeg" width="45%" alt="Pacientes" />
  <img src="screenshots/financeiro.jpeg" width="45%" alt="Financeiro" />
</p>
<p align="center">
  <img src="screenshots/configuracoes.jpeg" width="45%" alt="Configurações" />
</p>

## Instalação

O setup principal é feito via **GitHub Releases**. Baixe o instalador da
[última release](https://github.com/Clebson-Torres/AtendeMente/releases) para Windows, macOS ou Linux.

### Desenvolvimento

```bash
npm install
```

Configure variáveis de ambiente (ou use os defaults):

```env
DATABASE_URL=sqlite:C:/Users/you/.config/atendemente/atendemente.db?mode=rwc
AUTH_DATABASE_URL=sqlite:C:/Users/you/.config/atendemente/auth.db?mode=rwc
SERVER_PORT=3001
STORAGE_DIR=C:/Users/you/.config/atendemente/uploads
MASTER_PEPPER=base64-32-bytes
```

A chave mestra (`MASTER_PEPPER`) é opcional. Se não for fornecida, o sistema gera uma
de 32 bytes via CSPRNG e a armazena no Windows Credential Manager (ou macOS Keychain).

## Desenvolvimento

```bash
# Tauri desktop
npm run tauri dev

# Só frontend (API precisa rodar separada)
npm run dev

# Servidor standalone (para testar sem Tauri)
cd src-tauri && cargo run --bin server -- --port 3001
```

O frontend espera a API em `http://localhost:3001/api`.

## Testes

```bash
# Unitários + integração (Vitest)
npm run test

# Rust
cd src-tauri && cargo test

# E2E (Playwright) — local apenas
npm run test:e2e
```

## Build

```bash
# Frontend
npm run build

# Desktop (gera instalador)
npm run tauri build

# Servidor standalone
cd src-tauri && cargo build --bin server --release
```

## Estrutura do Projeto

```
screenshots/      Imagens do app (README)
src/
  components/     Componentes React (UI, Layout, LockScreen, onboarding, security)
  pages/          Telas (Login, Register, OnboardingFlow, Dashboard, Patients, Appointments, Payments)
  lib/            Helpers (auth.ts, api.ts, utils.ts, format.ts)

src-tauri/
  src/
    api/          Rotas Axum (routes.rs)
    auth/         Autenticação (mod.rs, auth_service.rs, tests.rs)
    features/     Lógica de negócio (patients, appointments, payments, records, backup, dashboard)
    db/           SQLite (init, models, migrations)
    crypto.rs     AES-256-GCM + HKDF
    config.rs     Config + keychain loading
    middleware.rs Headers de segurança
    rate_limit.rs Rate limiting por escopo
  migrations/     SQL migrations
  icons/          Ícones do app

e2e/              Testes Playwright (specs, fixtures, runner)
tests/            Testes Vitest (auth, appointments, schemas, format, cn, form, ...)
```

## CI

O workflow do GitHub Actions executa:

1. **quality** (ubuntu-latest)
   - TypeScript check
   - Testes unitários + integração (Vitest)
   - Build do frontend

2. **build** (ubuntu, windows, macos — após quality)
   - Compilação do backend Rust
   - Build do frontend
   - Verificação do binário gerado

## Licença

Proprietária — todos os direitos reservados.
