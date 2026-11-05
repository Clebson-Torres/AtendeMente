# Análise do AtendeMente: código limpo, segurança e melhorias

**Versão analisada:** v1.1.0 (commit `daacb65`).
**Branch das correções:** `analise/bateria-2026-11`.
**Data:** 2026-11-05.

## Sumário executivo

O projeto tem uma base boa:
- isolamento por usuário bem feito (um banco por usuário e filtro por `user_id` em todas as consultas);
- criptografia correta no essencial (nonce aleatório, HKDF com separação de propósito, envelope de chave nos backups v3);
- testes Rust que exercitam compatibilidade de backup com fixtures reais;
- um CI que sobe o binário de verdade.

A auditoria anterior (F1–F11) se sustenta quase toda. Esta rodada encontrou **dois bugs de perda ou exposição de dados que nenhum teste cobria** e uma regressão de comportamento no bloqueio de tela.

| | Crítica | Alta | Média | Baixa / Info |
|---|---|---|---|---|
| Segurança | 1 (dependência) | 3 | 9 | 12 |
| Bugs funcionais | — | 3 | 4 | vários |
| Qualidade / CI | — | 4 | ~15 | ~15 |

**Corrigido nesta rodada** (6 commits, cada um com teste que falhava antes):

| Commit | O quê |
|---|---|
| `8819a37` | Q1: o prontuário nunca era carregado, e "Salvar" apagava o prontuário existente |
| `6a1a7f9` | S1: o restore instalava anexos clínicos **em texto claro** no disco |
| `41bdd75` | S2/S3: o bloqueio e o logout nunca chegavam ao servidor, e uma sessão bloqueada ainda exportava backup |
| `332b851` | S4–S6: DNS rebinding (checagem de `Host`), CORS amplo, rate limit de login contornável |
| `9428600` | S7: `dompurify` (crítica) e `zod` (alta) |
| `cb0bc2f` | S8: uma falha do cofre podia gerar um pepper novo por cima do real (perda irreversível) |

## Baseline: antes e depois

| Verificação | Antes | Depois |
|---|---|---|
| `npm run lint` (tsc) | ok | ok |
| Vitest | 131 ✔ | 134 ✔ |
| `npm run build` | ok | ok |
| `cargo test --lib` | 181 ✔ | 193 ✔ |
| `cargo clippy --all-targets` | 34 avisos | 33 (nenhum em código novo) |
| `cargo fmt --check` | 294 diferenças | inalterado (não formatado de propósito: o diff seria enorme e misturaria com as correções) |
| `npm audit --omit=dev` | 1 crítica, 1 alta, 2 moderadas | 2 moderadas (react-router, só na v7) |
| Smoke test (health, register, login) | ok | ok, mais: Host de rebinding → 421, origem de LAN sem CORS, lock → `/backup` 401 → unlock → 200 |

---

## 1. Segurança

Método: revisão focada por modelo de ameaça, em duas frentes:
- API em loopback, autenticação e webview;
- criptografia, backup, arquivos e sistema de arquivos.

Cada achado foi conferido lendo o fluxo do começo ao fim. Os marcados como corrigidos também foram confirmados por teste e, quando aplicável, contra o binário real.

O scan multiagente do Claude Security não rodou nesta sessão, porque exige ser o agente principal. Para rodá-lo: abrir uma sessão nova no repositório e executar `/claude-security`.

### Corrigidos

**S1. O restore grava os anexos em texto claro (alta).** `crypto.rs` `reencrypt_file_bytes`
- **Problema:** quando `old_key == new_key`, os bytes eram devolvidos intactos. Mas o backup feito após o login guarda os anexos já decifrados no ZIP. Resultado: restaurar na mesma máquina (o caso mais comum) instalava laudos e imagens em claro em `uploads/`, e o download os servia por um passthrough "legado". Backups v1 sem chave de origem também caíam em claro.
- **Correção:** sempre decifrar e recifrar. Sem chave de origem, cifrar com a chave atual o que estiver em claro.
- **Pendência:** instalações que já restauraram um backup têm arquivos em claro no disco. Recomendo uma varredura no login: todo arquivo em `uploads/<user>` que não comece com `0x01` é cifrado e regravado.

**S2. O bloqueio e o logout nunca chegavam ao servidor (alta).** `src/lib/auth.ts`
- **Problema:** `apiRequest` deduzia o método pela presença de corpo. `/auth/lock` e `/auth/logout` saíam como GET, recebiam 405, e o erro era engolido. Consequências:
  - a chave AES ficava no cache;
  - um F5 reabria o app desbloqueado;
  - o token "deslogado" seguia válido por 7 dias.
- Os testes afirmavam `GET`, o que consolidava o bug.
- **Correção:** método explícito. Se o lock falhar, o app faz logout (falha fechado). O conteúdo atrás do overlay fica `inert`.

**S3. Uma sessão bloqueada ainda acessava os dados (média).** `api/routes.rs`, `commands.rs`
- **Problema:** `POST /backup` deriva a chave do pepper, sem depender do cache. Com o token de uma sessão bloqueada era possível exportar o banco inteiro com uma senha escolhida pelo atacante.
- **Correção:** as rotas de dados e os comandos Tauri exigem a chave carregada (`require_unlocked`).

**S4. DNS rebinding na API em loopback (média).** `middleware.rs`
- **Problema:** nada validava o `Host`. Uma página em `rebind.atacante.com:3001`, com o DNS trocado para 127.0.0.1, ficava same-origin e podia chamar login, register e recover.
- **Correção:** `Host` fora de loopback é recusado com 421.

**S5. Rate limit de login contornável (média).** `auth/mod.rs`
- **Problema:** o limite era chaveado pelo e-mail cru. `Vitima@x.com` e ` vitima@x.com` caíam em contadores diferentes, o que permitia força bruta ilimitada.
- **Correção:** a chave do limite usa o e-mail normalizado.

**S6. CORS amplo demais (baixa).** `lib.rs`
- **Problema:** aceitava qualquer IP de rede privada e `*.localhost`, sobra do acesso mobile que foi removido.
- **Correção:** restrito ao shell do Tauri e aos endereços de loopback.

**S7. Dependências npm.**
- `dompurify` 3.4.11 (crítica, XSS, entra via jspdf) atualizada para 3.4.17.
- `zod` 4.4.3 (alta, DoS de memória) atualizado para 4.6.7.

**S8. Pepper sobrescrito após falha do cofre (média; o impacto é perda total).** `config.rs`
- **Problema:** qualquer erro do keyring virava "não existe", e o app gerava e gravava um pepper novo por cima do real.
- **Correção:** só `NoEntry` autoriza gerar. Os outros erros abortam com mensagem clara.

### Recomendados (não corrigidos nesta rodada)

| ID | Sev. | Achado | Onde | Correção sugerida |
|---|---|---|---|---|
| S9 | média | O restore apaga e substitui a trilha de auditoria LGPD e zera `request_limits` | `backup.rs` `RESTORE_TABLE_ORDER` | Tirar `audit_logs` e `request_limits` do restore. Se quiser preservar o histórico do backup, importar com `INSERT OR IGNORE` marcado como `origem=backup` |
| S10 | média | O pepper está exposto em backups v2 antigos e não existe rotação | `backup.rs:76`, sem rotina de rotação | Criar uma rotina de rotação do pepper (recifrar PII, prontuários e anexos e reindexar) e orientar o descarte dos v2 antigos |
| S11 | média | O pepper fica em claro no `config.toml` (fallback/migração), que é mantido para sempre; sem `chmod` 600/700 no Unix | `config.rs:262-301`, criação de diretórios | Apagar a chave do toml após migrar para o cofre; criar com `mode(0o600/0o700)` em Unix |
| S12 | média | Restaurações limitadas a ~15 MB (base64 em JSON, limite global de 20 MB); bomba de descompressão no ZIP | `lib.rs:386`, `routes.rs:254`, `read_zip_entry` | Restore via comando Tauri lendo o arquivo, ou limite por rota; `take(MAX)` por entrada e teto total |
| S13 | média | Sobra texto claro residual no SQLite (PII legada, tokens da migration 9) sem `secure_delete` | `db/mod.rs` | `PRAGMA secure_delete=ON` e um `VACUUM` único controlado por migration nova |
| S14 | média | `full_name`, `chart_number`, `quick_notes`, `cancel_reason` e `original_name` (na auditoria) ficam em claro | `patients.rs`, `appointments.rs`, `files.rs` | Mover para o blob cifrado (migration nova) e buscar nome só por n-gramas HMAC |
| S15 | baixa | O restore aceita `patient_id` e `storage_path` arbitrários, o que permite path traversal em download e exclusão | `backup.rs:858`, `files.rs:330,432` | Validar UUIDs e nome `<uuid>.<ext>`; canonicalizar e exigir `starts_with(storage/<user>)` |
| S16 | baixa | A porta 3001 é fixa e a falha de bind em `[::1]` é ignorada; outro processo local pode ocupá-la | `lib.rs:433`, `main.rs:114` | Falhar fechado e usar um segredo por execução entregue via comando Tauri; front apontando para `127.0.0.1` |
| S17 | baixa | O commit do import não chama `validate()` e o `session_id` não está ligado ao preview | `patients_import.rs:119` | `validate()` por linha, teto de linhas e transação |
| S18 | baixa | O conteúdo do anexo pode ser regravado após a confirmação, burlando o magic-byte | `files.rs:188` | Recusar a escrita depois de `confirmed_at`; checar o tamanho declarado |
| S19 | baixa | `decrypt_file` com passthrough usado no envelope e na camada ATND, que deixa de autenticar | `backup.rs:278,338` | Criar `decrypt_file_strict` (exige `0x01`) nesses dois pontos |
| S20 | baixa | O register é oráculo de "e-mail já cadastrado" e não tem teto global | `auth_service.rs:138` | App de uso local: permitir o register só sem contas, ou exigir sessão para criar outra. Um teto global quebraria o E2E (45 contas por execução) |
| S21 | baixa | CSP com `ws:` e `http://localhost:*` em `connect-src` | `tauri.conf.json` | `devCsp` para o dev e `connect-src 'self' ipc: http://ipc.localhost http://localhost:3001` em produção |
| S22 | info | `shell:allow-open` concedido e nunca usado | `capabilities/default.json`, `main.rs` | Remover a permissão e o plugin |
| S23 | info | Mínimo de **8** caracteres na senha da conta (o F4 tratava da senha de backup, que é 12); `len()` conta bytes | `auth_service.rs:121,431`, `schemas.ts` | Decisão de produto. Se quiser 12: uma constante única e `chars().count()` |
| S24 | info | Código de recuperação de 64 bits com SHA-256 sem sal; `dotenvy` lido em release; `key_version` decorativo; sem `zeroize`; sem AAD no GCM; parâmetros do Argon2 fora do manifesto | vários | Endurecimentos incrementais |
| S25 | moderada | react-router 6: open redirect via `<Link>`/`navigate` com destino controlado (o app não usa SSR) | `package.json` | Migrar para o react-router 7 |

**Verificado e ok:**
- **CSRF:** `Json` recusa `text/plain`/form, e as rotas de dados exigem Bearer.
- **Token:** UUIDv4 com `OsRng`, guardado como SHA-256, TTL de 7 dias.
- **Argon2id:** parâmetros OWASP; hash dummy para e-mail inexistente.
- **Recuperação:** tempo constante e uso único.
- **IDOR:** não encontrado.
- **Zip slip:** bloqueado por `safe_join`.
- **CSV:** sem injection (`escape_csv`).
- **Upload:** allowlist e magic bytes.
- **Fallback estático:** canonicaliza o caminho.
- **Frontend:** sem `dangerouslySetInnerHTML`/`eval`; os links do WhatsApp e do Meet não injetam dados.
- **F1, F2, F3, F6, F7, F8, F9 e F11:** se mantêm.

---

## 2. Bugs funcionais

| ID | Sev. | Achado | Onde | Situação |
|---|---|---|---|---|
| Q1 | **alta** | `loadRecord()` nunca era chamada: o editor de prontuário abria vazio e "Salvar" sobrescrevia o prontuário cifrado | `AppointmentDetail.tsx:89` | **Corrigido** `8819a37`. Um 404 abre o editor vazio; outro erro esconde o Salvar |
| Q2 | alta | `request()` perdia o erro real quando a resposta não era JSON ("body used already") | `api.ts` | **Corrigido** `8819a37` |
| Q3 | alta | `paid_at` é gravado em UTC (`toISOString`) e exibido cortando a string, o que adianta a hora em 3h e vira a data depois das 21h. Editar um pagamento troca o `paid_at` por "agora" | `AppointmentDetail.tsx:109`, `Payments.tsx:96`, `format.ts` | Recomendado: uma convenção única de fuso (front e back). O mesmo problema aparece em `dashboard.rs` ("hoje"/"próximos") e no agendador de backup |
| Q4 | média | O seletor de paciente no agendamento mostra só os 50 primeiros; `search`/`status` sem `encodeURIComponent` | `Appointments.tsx:89`, `api.ts:51` | Recomendado: combobox com busca e `URLSearchParams` |
| Q5 | média | `Payments.tsx:87`: `openEdit` zera `notes`; colunas com `key` duplicada no `DataTable`; `Settings.tsx:87` com condição duplicada; `Patients` busca duas vezes ao montar | vários | Recomendado (ganhos rápidos) |
| Q6 | média | Escritas em várias etapas sem transação: série recorrente, create/update de paciente mais índice mais auditoria, import, register (usuário órfão se o banco do app falhar), `rewrite_storage_paths` | `appointments.rs:306`, `patients.rs:858`, `auth/mod.rs:80`, `backup.rs:843` | Recomendado: fazer `audit` e `set_patient_tokens` aceitarem `&mut Transaction` |
| Q7 | média | O restore não é atômico entre banco e disco (o banco é trocado antes do storage) | `backup.rs:421` | Recomendado |
| Q8 | média | Panic com IV de tamanho errado (`Nonce::from_slice`) | `crypto.rs` | **Corrigido** `6a1a7f9` |

## 3. Código limpo e arquitetura

**Backend Rust**

| Sev. | Achado | Sugestão |
|---|---|---|
| alta | CPU bloqueante no runtime async: Argon2 (login/unlock/backup), ZIP+AES do backup inteiro em memória, keyring e `blocking_save_file` | Wrappers com `spawn_blocking` |
| alta | Detecção de duplicata carrega e decifra **todos** os pacientes a cada create; O(M·N) no import | Remover o fallback (o índice é garantido por fingerprint) e reindexar em uma transação |
| alta | `auth_service` devolve `Result<_, String>`: falha de SQL vira 401/400 e o texto interno vaza para o cliente | `enum AuthError` com `thiserror` e `From<AuthError> for AppError` |
| média | ~124 `map_err(\|e\| AppError::internal(format!(..)))`, 26 descartando a causa; `AppError::Crypto` e `Internal.source` sem uso | Trait `ResultExt::context()` e `?` direto em `sqlx::Error` |
| média | Duas APIs de auditoria; os "create" são gravados como `update` (`PatientCreated` nunca é emitido) | Migrar para `write_audit_event(AuditAction::…)` |
| média | Autenticação e abertura do banco repetidas em todo handler; extração do token triplicada | Extractors `AuthUser`/`UserDb` (`FromRequestParts`) |
| média | `get_or_open_user_db` com corrida (migrations duas vezes) e um `SELECT 1` por requisição; `PRAGMA` aplicado a uma única conexão do pool | `OnceCell` por usuário; `SqliteConnectOptions` com WAL e foreign_keys |
| média | `backup.rs` (~1.030 linhas de produção), `patients.rs` (~1.010) e `routes.rs` grandes demais | Dividir em `backup/{manifest,bundle,restore/{db_import,storage},schedule}` e `patients/{pii,search_index,duplicates,list}`, `api/` por domínio |
| média | `month_bounds` triplicado com `unwrap_or(2026)`; upsert via SELECT+INSERT com corrida | `utils::month_bounds` validado; `INSERT … ON CONFLICT` |
| baixa | Código morto: `RATE_LIMITS` (e limites literais espalhados), `ListPatientsQuery`, query de arquivos descartada no detalhe de agendamento, `create_backup`, `session_id` do import | Remover; `enum RateScope` |
| baixa | Mistura de PT/EN nos identificadores; mensagens ao usuário em inglês ("Decryption failed") | Identificadores em EN e mensagens em PT centralizadas |
| média | Sem testes HTTP do router, `upsert_payment`, `export_*`, `commit_import`, agendador e fallback SPA; fixtures `test_db` duplicadas em ~9 módulos | `mod test_support` compartilhado |

**Frontend**

| Sev. | Achado | Sugestão |
|---|---|---|
| média | Fetch sem cancelamento nem checagem de resposta obsoleta (trocar de mês rápido mostra o mês errado). Os `AbortController` existentes são decorativos (`signal` nunca chega ao `fetch`) | `request()` com `signal` e um hook `useApiQuery`/`useAction`, que também eliminam ~30 blocos `try/catch(e:any)` |
| média | Três clientes HTTP paralelos (`request`, `apiRequest` e `fetch` manuais); `isTauri` e base64 duplicados (`Settings.tsx` converte caractere a caractere) | Um único `http.ts` (`requestJson`/`requestBlob`) e `toBase64` |
| média | `Appointments.tsx` (421 linhas) e `AppointmentDetail.tsx` (423, 15 `useState`), `Login.tsx` (máquina de 4 estados com classes repetidas) | Extrair `CalendarGrid`, `MonthNavigator`, `PaymentCard`, `SessionRecordEditor`, `LoginForm`/`RecoverForm`/`ResetForm` |
| média | Validação fraca no zod: `ends_at > starts_at`, `until_date` condicional, RHF inconsistente (preço, recuperação e senha de backup validados à mão) | `superRefine` e todos os schemas em `schemas.ts` |
| média | Acessibilidade: o modal de upload não usa `<Modal>`; calendário e `<th>` ordenável sem teclado; `role=dialog` no overlay e `id` fixo; Toast sem `aria-live`; campos sem `<label>` | `<button>`, `useId`, `aria-sort`, `role=status/alert` |
| baixa | Só existe um `ErrorBoundary` global (falha de chunk lazy derruba o app); o PDF exporta só uma página; recharts entra no bundle inicial | Boundary por rota; paginar ou usar `window.print()`; gráficos `lazy` |

## 4. Repositório, CI e DX

| Sev. | Achado | Sugestão |
|---|---|---|
| alta | Sem ESLint/Prettier; `noUnusedLocals: false` (há imports mortos) | ESLint flat com `typescript-eslint`, `react-hooks` (`exhaustive-deps`) e `jsx-a11y` no job `quality` |
| alta | Release sem assinatura (`TAURI_SIGNING_PRIVATE_KEY: ""`, `certificateThumbprint: null`) e publicada direto (`draft: false`) | Assinatura Windows/macOS via secrets; publicar como draft |
| média | Actions em tags móveis, inclusive de terceiros com `contents: write` | Fixar por SHA e configurar Dependabot para `github-actions` |
| média | Sem cache de cargo (matriz de 3 SOs compila do zero, duas vezes); sem `permissions: contents: read` nem `concurrency` | `Swatinem/rust-cache@v2`, permissões mínimas, `cancel-in-progress` |
| média | Clippy (34 avisos), `fmt` (294 diferenças) e E2E fora do CI | Um PR só de `cargo fmt` e depois `clippy -D warnings`; job Linux com Playwright |
| baixa | `src-tauri/icon3.png` duplicado; `icons/icon1.png` (1,3 MB) e `icon2/icon3` sem uso; `icons/ios` e `android` num app desktop; `src-tauri/gen/schemas` versionado (e reescrito a cada build) | Remover e adicionar `src-tauri/gen/` ao `.gitignore` |
| baixa | README com contagem de testes fixa e desatualizada; screenshots desatualizadas | Remover os números ou gerá-los |

## 5. Ordem sugerida para as próximas rodadas

1. **Dados em risco:** varredura que recifra os anexos já restaurados em claro (S1), S9 (auditoria no restore), Q3 (fuso), S12 (restore > 15 MB).
2. **Robustez:** transações (Q6/Q7), `spawn_blocking`, `AuthError`.
3. **Higiene que destrava o resto:** `cargo fmt` em PR isolado, depois clippy e ESLint no CI, cache de cargo, actions fixadas, `src-tauri/gen` fora do git.
4. **LGPD a médio prazo:** S13/S14 (o que fica em claro), S10/S11 (pepper), S15–S19.
5. **Refatorações:** hooks de dados e cliente HTTP único no front; divisão de `backup.rs`/`patients.rs`/`routes.rs`.

## Verificação final

Rodado na branch após o último commit:

- `npm run lint`: ok
- Vitest: 134/134
- `npm run build`: ok
- `cargo test --lib --locked`: 193/193
- `cargo clippy --all-targets`: 33 avisos, todos pré-existentes
- Smoke test contra o binário `server` real:
  - `/health` 200;
  - `Host: rebind.evil.com` → 421;
  - preflight de `http://192.168.0.1` sem `Access-Control-Allow-Origin`, e `http://tauri.localhost` aceito;
  - register (senha de 11 caracteres) e login ok;
  - `/patients` 200;
  - `POST /auth/lock` 200;
  - `/auth/me` → `locked: true`;
  - `POST /backup` com sessão bloqueada → 401;
  - unlock 200;
  - `/patients` 200.

**Não verificado:**
- o app desktop empacotado (`tauri build`) e a suíte E2E Playwright não foram rodados;
- o comportamento do overlay `inert` e do F5 com a tela bloqueada não foi testado na webview real, só nos testes unitários e na API.
