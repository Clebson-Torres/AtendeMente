# Plano de Conversão: AtendeMente → Rust (Tauri v2 + Axum + SQLite)

## Stack Alvo

| Camada | Tecnologia |
|--------|-----------|
| Desktop Container | **Tauri v2** |
| Frontend | WebView (React ou Svelte) |
| Backend API | **Axum** (embarcado no Tauri) |
| Database | **SQLite** via **SQLx** |
| Autenticação | **Firebase Auth** (Admin SDK) |
| Criptografia | **aes-gcm** crate (AES-256-GCM) |
| File Storage | Sistema de arquivos local |
| Exportação | **zip** crate |
| CSV | **csv** crate |
| Validação | **validator** crate / custom |
| Serialização | **serde** + **serde_json** |

## Arquitetura

```
┌─────────────────────────────────────────────────┐
│                 Tauri v2 App                     │
│  ┌───────────────────────────────────────────┐   │
│  │  WebView Frontend (React/Svelte)          │   │
│  │  - Login (Firebase UI)                    │   │
│  │  - Dashboard                              │   │
│  │  - Pacientes CRUD                         │   │
│  │  - Agenda / Calendário                    │   │
│  │  - Prontuário criptografado               │   │
│  │  - Financeiro                             │   │
│  │  - Arquivos / Exportação                  │   │
│  └───────────────┬───────────────────────────┘   │
│                  │ HTTP localhost:PORT            │
│  ┌───────────────▼───────────────────────────┐   │
│  │  Axum Server (tokio task in background)   │   │
│  │                                            │   │
│  │  API REST (/api/*):                        │   │
│  │  - /api/health                            │   │
│  │  - /api/auth/*                            │   │
│  │  - /api/patients/*                        │   │
│  │  - /api/appointments/*                    │   │
│  │  - /api/payments/*                        │   │
│  │  - /api/records/*                         │   │
│  │  - /api/files/*                           │   │
│  │  - /api/exports/*                         │   │
│  │  - /api/dashboard                         │   │
│  │  - /api/calendar/events                   │   │
│  │                                            │   │
│  │  Middleware:                               │   │
│  │  - Firebase JWT verification               │   │
│  │  - Rate limiting                           │   │
│  │  - Security headers (CSP)                  │   │
│  │  - Audit logging                           │   │
│  │  - CORS (para dev)                         │   │
│  └───────────────┬───────────────────────────┘   │
│                  │                                │
│  ┌───────────────▼───────────────────────────┐   │
│  │  SQLite Database (atendemente.db)         │   │
│  │  + Local File Storage (uploads/)          │   │
│  └───────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## Estrutura de Diretórios

```
atendemente/
├── src-tauri/                    # Projeto Tauri (Rust)
│   ├── src/
│   │   ├── main.rs               # Entrypoint: Tauri + Axum
│   │   ├── api/
│   │   │   ├── mod.rs            # Router principal
│   │   │   ├── auth.rs           # Rotas de autenticação
│   │   │   ├── patients.rs       # Rotas de pacientes
│   │   │   ├── appointments.rs   # Rotas de agendamentos
│   │   │   ├── payments.rs       # Rotas de pagamentos
│   │   │   ├── records.rs        # Rotas de prontuários
│   │   │   ├── files.rs          # Rotas de arquivos
│   │   │   ├── exports.rs        # Rotas de exportação
│   │   │   ├── dashboard.rs      # Rotas do dashboard
│   │   │   └── calendar.rs       # Rotas do calendário
│   │   ├── db/
│   │   │   ├── mod.rs            # Pool SQLite + init
│   │   │   ├── models.rs         # Structs das tabelas
│   │   │   └── migrations/
│   │   │       ├── 001_init.sql
│   │   │       ├── 002_patient_profile.sql
│   │   │       ├── 003_payment_receipts.sql
│   │   │       ├── 004_appointment_confirmation.sql
│   │   │       ├── 005_recurring_series.sql
│   │   │       ├── 006_rate_limits.sql
│   │   │       └── 007_chart_number.sql
│   │   ├── features/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs           # Firebase token verification
│   │   │   ├── patients.rs       # Business logic: patients
│   │   │   ├── appointments.rs   # Business logic: appointments
│   │   │   ├── appointments_recurrence.rs  # Lógica de recorrência
│   │   │   ├── payments.rs       # Business logic: payments
│   │   │   ├── records.rs        # Business logic: session records
│   │   │   ├── files.rs          # Business logic: file upload/download
│   │   │   ├── exports.rs        # Business logic: ZIP export
│   │   │   └── dashboard.rs      # Business logic: dashboard metrics
│   │   ├── lib/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs           # require_user(), get_current_user()
│   │   │   ├── crypto.rs         # AES-256-GCM encrypt/decrypt
│   │   │   ├── audit.rs          # Audit logging
│   │   │   ├── rate_limit.rs     # Rate limiting
│   │   │   ├── errors.rs         # AppError, error handling
│   │   │   ├── utils.rs          # CN, formatadores, etc.
│   │   │   └── firebase.rs       # Firebase Admin SDK client
│   │   └── config.rs             # Config management
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   └── capabilities/
├── src/                          # Frontend (React/Svelte)
│   ├── App.svelte
│   ├── main.ts
│   ├── lib/
│   │   ├── api.ts                # API client
│   │   └── firebase.ts           # Firebase client SDK
│   └── routes/
│       ├── Login.svelte
│       ├── Dashboard.svelte
│       ├── Patients.svelte
│       ├── Agenda.svelte
│       ├── Appointments.svelte
│       └── ...
├── package.json
├── vite.config.ts
├── svelte.config.js
└── tsconfig.json
```

## Tabelas SQLite

### `users`
| Coluna | Tipo SQLite | Notas |
|--------|-------------|-------|
| id | TEXT PK | Firebase UID |
| email | TEXT NOT NULL | |
| full_name | TEXT | |
| two_factor_enabled | INTEGER DEFAULT 0 | SQLite bool |
| created_at | TEXT DEFAULT datetime('now') | ISO 8601 |
| updated_at | TEXT DEFAULT datetime('now') | ISO 8601 |

### `patients`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | NOT NULL |
| full_name | TEXT NOT NULL | |
| chart_number | TEXT | Nº prontuário |
| phone | TEXT | |
| email | TEXT | |
| birth_date | TEXT | ISO date |
| status | TEXT DEFAULT 'active' | CHECK('active','inactive') |
| health_history | TEXT | |
| medications_in_use | TEXT | |
| emergency_phone | TEXT | |
| admin_notes | TEXT | |
| created_at | TEXT | ISO 8601 |
| updated_at | TEXT | ISO 8601 |
| deleted_at | TEXT | Soft delete |

### `appointments`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| patient_id | TEXT FK→patients | |
| series_id | TEXT FK→recurring_series | Opcional |
| starts_at | TEXT NOT NULL | ISO 8601 |
| ends_at | TEXT NOT NULL | ISO 8601 |
| status | TEXT DEFAULT 'scheduled' | CHECK('scheduled','completed','cancelled','no_show') |
| confirmation_status | TEXT DEFAULT 'unconfirmed' | CHECK('unconfirmed','confirmed','cancelled') |
| session_price_cents | INTEGER DEFAULT 0 | |
| quick_notes | TEXT | |
| cancel_reason | TEXT | |
| created_at | TEXT | |
| updated_at | TEXT | |
| deleted_at | TEXT | |

### `payments`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| appointment_id | TEXT FK→appointments | UNIQUE |
| status | TEXT DEFAULT 'pending' | CHECK('pending','paid','cancelled') |
| method | TEXT DEFAULT 'other' | CHECK('pix','cash','card','bank_transfer','other') |
| paid_at | TEXT | ISO 8601 |
| amount_received_cents | INTEGER DEFAULT 0 | |
| notes | TEXT | |
| created_at | TEXT | |
| updated_at | TEXT | |
| deleted_at | TEXT | |

### `session_records`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| patient_id | TEXT FK→patients | |
| appointment_id | TEXT FK→appointments | UNIQUE |
| encrypted_payload | TEXT NOT NULL | Base64 |
| iv | TEXT NOT NULL | Base64 |
| auth_tag | TEXT NOT NULL | Base64 |
| key_version | INTEGER DEFAULT 1 | |
| created_at | TEXT | |
| updated_at | TEXT | |
| deleted_at | TEXT | |

### `record_files`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| patient_id | TEXT FK→patients | |
| appointment_id | TEXT FK→appointments | |
| payment_id | TEXT FK→payments | Opcional |
| kind | TEXT DEFAULT 'session_attachment' | CHECK('session_attachment','payment_receipt') |
| storage_path | TEXT NOT NULL | |
| original_name | TEXT NOT NULL | |
| mime_type | TEXT NOT NULL | |
| byte_size | INTEGER NOT NULL | |
| uploaded_at | TEXT | |
| deleted_at | TEXT | |

### `recurring_series`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| patient_id | TEXT FK→patients | |
| frequency | TEXT NOT NULL | CHECK('weekly','biweekly') |
| starts_on | TEXT NOT NULL | ISO date |
| ends_on | TEXT | |
| occurrences_count | INTEGER | |
| start_time | TEXT NOT NULL | HH:mm |
| end_time | TEXT NOT NULL | HH:mm |
| cancelled_at | TEXT | |
| created_at | TEXT | |
| updated_at | TEXT | |

### `audit_logs`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT FK→users | |
| action | TEXT NOT NULL | CHECK('login','logout','file_upload',...) |
| entity_type | TEXT NOT NULL | |
| entity_id | TEXT | |
| ip_address | TEXT | |
| user_agent | TEXT | |
| metadata | TEXT DEFAULT '{}' | JSON string |
| created_at | TEXT | |

### `request_limits`
| Coluna | Tipo | Notas |
|--------|------|-------|
| id | TEXT PK | UUID v4 |
| scope | TEXT NOT NULL | |
| identifier | TEXT NOT NULL | |
| hits | INTEGER DEFAULT 0 | |
| window_starts_at | TEXT | |
| created_at | TEXT | |
| updated_at | TEXT | |

## API Endpoints

```
GET    /api/health

# Auth (Firebase - token verification only)
POST   /api/auth/verify           # Verify Firebase token
POST   /api/auth/logout           # Log audit

# Patients
GET    /api/patients              # List (with search)
POST   /api/patients              # Create
GET    /api/patients/:id          # Detail
PUT    /api/patients/:id          # Update
POST   /api/patients/:id/activate
POST   /api/patients/:id/deactivate
POST   /api/patients/import/preview
POST   /api/patients/import/commit

# Appointments
GET    /api/appointments          # List upcoming
POST   /api/appointments          # Create
GET    /api/appointments/:id      # Detail
PUT    /api/appointments/:id      # Update
POST   /api/appointments/:id/cancel
POST   /api/appointments/series/:id/cancel

# Calendar
GET    /api/calendar/events?start=&end=

# Payments
POST   /api/payments/upsert       # Create or update
GET    /api/payments              # List all
GET    /api/payments/pending      # List pending
GET    /api/payments/summary      # Monthly summary

# Session Records
POST   /api/records/save          # Save (encrypted)
GET    /api/records/:appointment_id  # Read (decrypted)

# Files
POST   /api/files/upload-session  # Create upload session
POST   /api/files/confirm         # Confirm upload
GET    /api/files/:id/download    # Download
POST   /api/files/:id/delete      # Soft delete

# Exports
POST   /api/exports/patient/:id   # Generate ZIP

# Dashboard
GET    /api/dashboard             # Monthly stats
```

## Fases de Implementação

### Fase 1: Projeto e Infraestrutura (~15 arquivos)
- `cargo init` com Tauri v2 + Axum + SQLx
- Configuração Cargo.toml com dependências
- Configuração Tauri (tauri.conf.json, capabilities)
- Configuração de ambiente (config.rs)
- Docker/SQLite init

### Fase 2: Database Layer (~10 arquivos)
- Migrations SQLx (7 arquivos)
- Models Rust (structs com serde)
- Pool management (db/mod.rs)
- Query helpers

### Fase 3: Core Libraries (~8 arquivos)
- error handling (AppError + IntoResponse)
- Firebase Auth verification
- AES-256-GCM crypto
- Audit logging
- Rate limiting
- Utilitários (formatadores, etc.)

### Fase 4: Features Layer (~15 arquivos)
- Patients CRUD + search + import
- Appointments CRUD + overlap check + recurrence
- Payments upsert + summaries
- Session records encrypt/save/decrypt
- File upload/download/confirm
- ZIP export
- Dashboard metrics

### Fase 5: API Layer (~12 arquivos)
- Router principal com todas as rotas
- Middleware: auth, rate limit, CSP
- Handlers para cada recurso
- JSON responses com ActionResponse pattern

### Fase 6: Frontend (~20+ arquivos)
- Tauri webview setup
- Login com Firebase Auth
- CRUD UI components
- Calendário (FullCalendar ou similar)
- Dashboard
- Formulários

### Fase 7: Tests & Polish (~10 arquivos)
- Testes unitários (validações, crypto, recorrência)
- Testes de integração (API endpoints)
- Seed data script
- Documentação

## Dependências Cargo.toml

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-build = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
axum = { version = "0.8", features = ["macros"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "fs"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
aes-gcm = "0.10"
rand = "0.8"
base64 = "0.22"
zip = "2"
csv = "1"
validator = { version = "0.19", features = ["derive"] }
firebase-auth = "0.6"       # ou similar para Firebase token verification
jsonwebtoken = "9"          # para decodificar JWT do Firebase
reqwest = { version = "0.12", features = ["json"] }
log = "0.4"
env_logger = "0.11"
```

## Padrão de Código

### ActionResponse (como no original)
```rust
#[derive(Serialize)]
pub struct ActionResponse<T: Serialize = ()> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}
```

### AppError
```rust
pub struct AppError {
    pub message: String,
    pub status_code: StatusCode,
    pub code: String,
}

impl IntoResponse for AppError { ... }
```

### require_user (middleware pattern)
```rust
pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
}

pub async fn require_user(auth_header: Option<String>) -> Result<AuthenticatedUser, AppError> { ... }
```

### API Handler Pattern
```rust
async fn list_patients(
    Extension(db): Extension<SqlitePool>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<ListPatientsParams>,
) -> Result<Json<ActionResponse<Vec<Patient>>>, AppError> {
    let patients = patients_service::list(&db, &user.id, &params.search).await?;
    Ok(Json(ActionResponse::success(patients)))
}

---

## Estratégia de Testes

### Stack de Testes

| Camada | Ferramenta | O que testa |
|--------|-----------|-------------|
| **Unitários** | `#[cfg(test)]` + `cargo test` | Validações, criptografia, recorrência, utils, schemas |
| **Integração** | In-memory SQLite + Axum `TestClient` | API endpoints, auth flow, rate limit, audit |
| **E2E** | Tauri test harness / Playwright | Fluxo completo desktop |

### Estrutura de Testes

```
src/
  features/
    patients.rs              # código de produção
    patients_test.rs         # unit tests (inline #[cfg(test)])
  api/
    appointments.rs
    appointments_test.rs     # integration tests
  lib/
    crypto.rs
    crypto_test.rs
    utils.rs
    utils_test.rs
tests/                       # Integration tests
  api/
    mod.rs                   # Test helpers: setup_test_db(), create_test_user()
    patients_test.rs
    appointments_test.rs
    payments_test.rs
    auth_test.rs
    exports_test.rs
  common/
    mod.rs                   # Shared fixtures & helpers
    fixtures.rs              # Factory functions for test data
```

### Testes Unitários (inline `#[cfg(test)]`)

**lib/crypto.rs**
- `test_encrypt_decrypt_roundtrip` — texto → encrypt → decrypt → texto original
- `test_encrypt_different_iv` — mesmo texto gera payloads diferentes
- `test_decrypt_wrong_key_fails` — chave errada = erro
- `test_decrypt_tampered_payload_fails` — payload adulterado = erro

**lib/utils.rs**
- `test_format_currency_brl` — R$ 1.234,56
- `test_parse_brl_to_cents` — "R$ 1.234,56" → 123456
- `test_format_phone` — "(11) 99999-8888"
- `test_normalize_patient_name` — acentos removidos
- `test_build_patient_identity_key` — nome+phone → chave única

**features/appointments_recurrence.rs**
- `test_build_recurring_weekly` — 4 semanas
- `test_build_recurring_biweekly` — 3 sessões
- `test_recurrence_until_date` — até data limite
- `test_recurrence_hard_limit_52` — máximo 52 sessões
- `test_recurrence_min_2_sessions` — valida mínimo

**features/patients/import.rs**
- `test_parse_csv_valid` — CSV bem formado
- `test_parse_csv_missing_name` — linha sem nome
- `test_detect_duplicates_in_file` — duplicatas no CSV
- `test_parse_csv_header_aliases` — apelidos de cabeçalho

**features/patients/schemas.rs**
- `test_patient_form_valid` — dados válidos
- `test_patient_form_empty_name` — nome vazio
- `test_patient_form_invalid_email` — email inválido

**features/appointments/schemas.rs**
- `test_appointment_form_valid`
- `test_appointment_end_before_start`
- `test_appointment_recurrence_needs_end_mode`
- `test_appointment_recurrence_invalid_occurrences`

**features/files/schemas.rs**
- `test_allowed_extension` — .pdf ok, .exe rejeitado
- `test_mime_type_matches_extension` — .pdf + application/pdf ok
- `test_file_upload_max_size` — 10MB limite

### Testes de Integração (tests/api/)

Setup compartilhado em `tests/common/mod.rs`:

```rust
pub async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}

pub fn create_test_app(pool: SqlitePool) -> Router {
    // Inicializa Axum router com banco de teste
    let state = AppState { db: pool };
    build_router(state)
}

pub async fn create_test_user(db: &SqlitePool) -> AuthenticatedUser {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO users (id, email, full_name) VALUES (?, ?, ?)")
        .bind(&id)
        .bind("test@test.com")
        .bind("Test User")
        .execute(db)
        .await
        .unwrap();
    AuthenticatedUser { id, email: "test@test.com".into(), full_name: Some("Test User".into()) }
}
```

**Testes de API:**

- `POST /api/patients` → `GET /api/patients` → `GET /api/patients/:id`
- `POST /api/appointments` com overlap detection
- `POST /api/appointments/:id/cancel` com audit log
- `POST /api/payments/upsert` com upsert
- `POST /api/records/save` → criptografia + leitura
- `POST /api/files/upload-session` → validacao
- `POST /api/exports/patient/:id` → ZIP gerado
- `GET /api/dashboard` → métricas do mês
- Rate limit: exceder limite retorna 429
- Auth: sem token retorna 401

### Comandos de Teste

```bash
# Todos os testes
cargo test

# Só unitários (rápido)
cargo test --lib

# Só integração
cargo test --test '*'

# Com output
cargo test -- --nocapture

# Com logging
RUST_LOG=debug cargo test
```

### Cobertura Alvo

| Módulo | Testes | Cobertura |
|--------|--------|-----------|
| lib/crypto | 4 | 95%+ |
| lib/utils | 6 | 90%+ |
| lib/rate_limit | 2 | 80%+ |
| lib/audit | 1 | 80%+ |
| features/patients/schemas | 3 | 100% validação |
| features/appointments/schemas | 4 | 100% validação |
| features/appointments/recurrence | 5 | 95%+ |
| features/patients/import | 4 | 90%+ |
| features/files/schemas | 3 | 90%+ |
| api/patients | 5 | 80%+ |
| api/appointments | 5 | 80%+ |
| api/payments | 3 | 80%+ |
| api/records | 2 | 80%+ |
| api/auth | 2 | 80%+ |
| Total | ~50+ | 85%+ |

### CI (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo build
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
```
```
