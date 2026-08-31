# Issues de segurança — AtendeMente v1.0.19

Geradas a partir do relatório de auditoria de 31 de agosto de 2026 (commit 6ed2d6a).

--- ISSUE 1 ---

## [Segurança] PII de contato em texto claro em patient_search_tokens

**Labels:** `security, alta, lgpd`
**Severidade:** Alta
**Categoria:** 1. Banco sem tranca (isolamento por usuário)

### Problema

A migration 20240101000008 moveu a PII do paciente para um blob AES-256-GCM (`pii_encrypted`) e as colunas em claro passaram a ser gravadas como NULL. A migration 9 devolveu telefone e e-mail ao disco em texto claro, agora em outra tabela, para permitir busca por substring. O resultado é que os dois campos de contato mais identificáveis do prontuário estão legíveis para quem abrir o arquivo .db com qualquer cliente SQLite — sem senha, sem o pepper, sem passar pelo cofre de credenciais do sistema operacional. E o arquivo .db é justamente o que sai da máquina: backups em pen drive, pastas sincronizadas com a nuvem, um HD retirado de um notebook roubado. Além disso, `patient_search_tokens` é a única tabela de dados sem coluna `user_id`: o escopo de dono existe apenas indiretamente, pelo JOIN com `patients`. Hoje isso não vaza — as consultas de busca em patients.rs:360-396 prendem o resultado com um WHERE user_id externo — mas a proteção passa a depender de todo autor futuro lembrar de escrever o JOIN.

### Evidência

- `src-tauri/src/db/migrations/20240101000009_search_tokens.sql:1-5`
- `src-tauri/src/features/patients.rs:236-252`
- `src-tauri/src/features/patients.rs:257-277`

```
-- migrations/20240101000009_search_tokens.sql:1-5
CREATE TABLE IF NOT EXISTS patient_search_tokens (
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    token_type TEXT NOT NULL,
    token_text TEXT NOT NULL          -- <- sem user_id, e o valor vai em claro
);

// features/patients.rs:236-252
if let Some(phone) = &pii.phone {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        tokens.push((patient_id.to_string(), "phone".to_string(), digits));
    }
}
if let Some(email) = &pii.email {
    let lower = email.to_lowercase();
    tokens.push((patient_id.to_string(), "email".to_string(), lower.clone()));
```

### Impacto

Exposição de dados pessoais de pacientes (LGPD, art. 46) a partir de uma cópia do arquivo de banco, anulando na prática a criptografia de PII para telefone e e-mail.

**Condições de exploração:** Não requer nenhuma configuração especial. Explorável por qualquer um com acesso de leitura ao arquivo `<DATA_DIR>/<user_id>/atendemente.db`.

### Correção sugerida

Guardar um token derivado irreversível em vez do valor: por exemplo HMAC-SHA256 truncado por n-grama do telefone e do e-mail, com chave derivada do pepper. A busca por substring passa a gerar os mesmos n-gramas a partir do termo consultado. Se a busca em claro for considerada requisito inegociável, o mínimo é adicionar a coluna `user_id` à tabela e ao WHERE, para que o isolamento não dependa do JOIN.

### Critérios de aceite

- [ ] `patient_search_tokens.token_text` não contém mais telefone nem e-mail legíveis (verificável com `SELECT token_text FROM patient_search_tokens` num banco de teste)
- [ ] A tabela tem coluna `user_id NOT NULL` e todas as consultas que a leem filtram por ela
- [ ] Busca por telefone e por e-mail continua funcionando (teste de integração cobrindo busca parcial)
- [ ] Existe migration que reindexa as linhas já gravadas em claro

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F1, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — migration 15 substitui os valores em claro por HMAC-SHA256 de n-gramas, com chave derivada do pepper, e a tabela ganhou `user_id`.

--- FIM ISSUE 1 ---

--- ISSUE 2 ---

## [Segurança] Senha de backup automático é global e ignora o usuário autenticado

**Labels:** `security, alta, idor`
**Severidade:** Alta
**Categoria:** 3. IDOR (objeto acessado sem verificar o dono)

### Problema

Toda a aplicação foi construída em torno do isolamento por usuário — banco separado, chave AES separada, `user_id` em cada WHERE — e este recurso escapa dele. As três rotas de `/api/backup/password` autenticam o chamador e então jogam fora o `user.id`: o `_` em `let _user` é a assinatura da falha, porque o alvo da operação não é derivado de quem chamou. Numa instalação com mais de uma conta — o app permite registrar quantas quiser, e o próprio scheduler itera sobre `SELECT id FROM auth_users` — isso produz três abusos concretos: (a) o usuário B chama `PUT /api/backup/password` e passa a conhecer a senha que cifra os backups automáticos `de A`, bastando obter o arquivo .atendemente para ler o prontuário completo de A; (b) o usuário B chama `DELETE` e os backups automáticos de A param de ser gerados em silêncio — lib.rs:237 apenas registra um warn e segue, enquanto a tela de A continua mostrando a frequência configurada; (c) A e B não conseguem manter senhas distintas: quem gravar por último vence.

### Evidência

- `src-tauri/src/config.rs:9`
- `src-tauri/src/config.rs:89-119`
- `src-tauri/src/api/routes.rs:190`
- `src-tauri/src/api/routes.rs:204`
- `src-tauri/src/api/routes.rs:215`
- `src-tauri/src/lib.rs:237`

```
// config.rs:9 — conta fixa, sem user_id no nome
const KEYCHAIN_BACKUP_PASSWORD: &str = "backup_password";

// config.rs:89-94
pub fn load_backup_password() -> Option<String> {
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_BACKUP_PASSWORD)   // <- mesma entrada para todos
        .ok()
        .and_then(|entry| entry.get_password().ok())

// api/routes.rs:204 e 215 — o usuário é autenticado e em seguida descartado
async fn set_backup_password(...) {
    let _user = get_authenticated_user(&headers, &state).await?;
    crate::config::save_backup_password(&input.password).map_err(AppError::bad_request)?;

async fn clear_backup_password(...) {
    let _user = get_authenticated_user(&headers, &state).await?;
    crate::config::delete_backup_password().map_err(AppError::internal)?;

// lib.rs:237 — dentro de `for user_id in &users`
let Some(password) = config::load_backup_password() else { ... continue; };
```

### Impacto

Quebra do isolamento entre contas para o artefato mais sensível do sistema (o backup contém o banco inteiro mais os anexos) e perda silenciosa de backup para o usuário que teve a senha removida por outro.

**Condições de exploração:** Explorável quando a instalação tem mais de uma conta registrada. Com conta única o impacto se reduz, mas o defeito de projeto permanece e vira falha assim que a segunda conta é criada.

### Correção sugerida

Tornar a entrada do cofre específica por usuário — por exemplo `Entry::new(KEYCHAIN_SERVICE, &format!("backup_password:{user_id}"))` — e propagar o `user.id` autenticado por `load/save/delete_backup_password` e pelo laço do scheduler em lib.rs. Trocar `let _user` por `let user` nos três handlers deixa o compilador cobrar o resto.

### Critérios de aceite

- [ ] A entrada do cofre inclui o `user_id` no nome da conta
- [ ] `load_backup_password`, `save_backup_password` e `delete_backup_password` recebem `user_id`
- [ ] Os três handlers de `/api/backup/password` usam o `user.id` do token, não `_user`
- [ ] O scheduler em `lib.rs` busca a senha do usuário da iteração
- [ ] Teste: com duas contas, definir a senha em A não altera nem remove a de B

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F2, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — a entrada do cofre passou a incluir o `user_id`, as tres rotas usam o usuario autenticado e a senha legada e migrada no boot.

--- FIM ISSUE 2 ---

--- ISSUE 3 ---

## [Segurança] Master pepper é serializado dentro do manifesto do backup

**Labels:** `security, alta, crypto`
**Severidade:** Alta
**Categoria:** 4. Chaves expostas

### Problema

O pepper existe justamente para `não` sair da máquina: é gerado por CSPRNG na primeira execução e guardado no cofre de credenciais do sistema operacional (config.rs:146-229). Gravá-lo no backup transfere esse segredo para um arquivo feito para ser copiado — pen drive, e-mail, pasta sincronizada. O comentário da linha 133 reconhece o risco e o considera coberto pela cifragem do ZIP, mas isso rebaixa a proteção do pepper de "cofre do sistema operacional" para "a senha que o usuário escolheu", e o servidor aceita senha de `8 caracteres` nesse ponto (backup.rs:25 — ver F4). Como `derive_user_key` é apenas `HKDF(pepper, user_id)` e o `user_id` viaja em claro no mesmo manifesto, quem quebrar a senha do backup não obtém apenas os dados daquele usuário: obtém a chave de `todos` os usuários daquela instalação, e ela continua válida para os bancos que permanecem na máquina. O motivo funcional é legítimo — restaurar num computador novo, onde o cofre não existe mais — mas a solução escolhida embute a chave-raiz no artefato.

### Evidência

- `src-tauri/src/features/backup.rs:61`
- `src-tauri/src/features/backup.rs:128-137`
- `src-tauri/src/features/backup.rs:148`
- `src-tauri/src/crypto.rs:46-52`

```
// features/backup.rs:128-137
let (salt_hex, pepper_field, pepper_fp) = if is_encrypted {
    let mut salt = [0u8; SALT_SIZE];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let fp = crypto::pepper_fingerprint().ok();
    let pepper_hex = crypto::get_pepper().map(|p| hex_encode(p));   // <- 32 bytes em hex
    // pepper_hex is only stored inside the encrypted ZIP (protected by password)
    (Some(hex_encode(&salt)), pepper_hex, fp)

// features/backup.rs:148 — vai para o manifest.json dentro do ZIP
    pepper: pepper_field,

// crypto.rs:46-52 — o que esse valor destranca
fn derive_key_inner(user_id: &str, pepper: &[u8; 32]) -> Result<[u8; 32], AppError> {
    let hk = Hkdf::<Sha256>::new(Some(pepper), user_id.as_bytes());
```

### Impacto

O comprometimento de um único arquivo de backup, com a senha quebrada, expõe a chave-raiz de criptografia da instalação inteira — inclusive de contas cujo backup nunca vazou.

**Condições de exploração:** Presente em todo backup cifrado: o campo só é preenchido quando `is_encrypted`. A exploração exige obter o arquivo e quebrar a senha escolhida pelo usuário.

### Correção sugerida

Não transportar o pepper. Cifrar a chave por usuário com uma chave derivada da senha do backup (Argon2id) e guardar apenas esse envelope no manifesto, de modo que o segredo transportado seja função da senha e não a chave-raiz da máquina. Enquanto isso não for feito: elevar o mínimo de senha (F4), manter o `pepper_fingerprint` — que já existe e basta para detectar divergência de chave — e remover o campo `pepper`, passando a exigir o pepper antigo do usuário na restauração entre máquinas.

### Critérios de aceite

- [ ] `manifest.json` de um backup recém-criado não contém material de chave no campo `pepper`
- [ ] Restauração numa máquina com pepper diferente continua funcionando (teste de integração, reaproveitando as fixtures de `src-tauri/tests/fixtures/`)
- [ ] Backups já emitidos no formato antigo continuam restauráveis
- [ ] O `pepper_fingerprint` segue presente para detecção de divergência

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F3, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — formato de backup v3: o manifesto carrega um envelope da chave do usuario cifrado pela senha, e nao mais o pepper mestre. Bundles v1 e v2 continuam restaurando.

--- FIM ISSUE 3 ---

--- ISSUE 4 ---

## [Segurança] Mínimo de senha de backup diverge entre cliente (12) e servidor (8)

**Labels:** `security, media`
**Severidade:** Média
**Categoria:** 2. Regra definida no navegador

### Problema

A regra que o usuário enxerga é imposta apenas em React. Um `POST /api/backup` feito fora da interface — com curl, num script de automação, ou por qualquer código que passe a chamar a API depois — produz um backup válido protegido por 8 caracteres. E esse backup é o bundle completo: banco de pacientes, prontuários de sessão, anexos e, hoje, o pepper mestre (F3). A própria base de código já discorda de si mesma: `MIN_AUTO_BACKUP_PASSWORD_LEN = 12` em config.rs para a senha dos backups automáticos, contra `MIN_BACKUP_PASSWORD_LEN = 8` para o manual. E em nenhum dos dois caminhos há qualquer exigência além do comprimento.

### Evidência

- `src/pages/Settings.tsx:55`
- `src/pages/Settings.tsx:104`
- `src/pages/Settings.tsx:154`
- `src-tauri/src/features/backup.rs:25`
- `src-tauri/src/features/backup.rs:88-95`
- `src-tauri/src/config.rs:12`

```
// src/pages/Settings.tsx:55 — o que o usuário vê
if (pw.length < 12) {
  setPasswordError("Minimo 12 caracteres para backup com senha.");
  return;
}

// src-tauri/src/features/backup.rs:25 e 88-95 — o que o servidor aceita
const MIN_BACKUP_PASSWORD_LEN: usize = 8;
...
if let Some(pass) = password {
    if pass.chars().count() < MIN_BACKUP_PASSWORD_LEN {
        return Err(AppError::bad_request(format!(
            "A senha do backup deve ter no mínimo {} caracteres.", MIN_BACKUP_PASSWORD_LEN)));

// src-tauri/src/config.rs:12 — a senha automática, por outro lado, exige 12
pub const MIN_AUTO_BACKUP_PASSWORD_LEN: usize = 12;
```

### Impacto

Backups contendo o prontuário completo podem ser gerados com senha muito mais fraca do que a aplicação promete, reduzindo o custo de um ataque offline sobre o arquivo — e, por tabela, sobre a chave-raiz que ele carrega.

**Condições de exploração:** Explorável por qualquer chamada à API fora da interface. Não requer configuração insegura.

### Correção sugerida

Elevar `MIN_BACKUP_PASSWORD_LEN` ao mesmo valor da constante de config.rs — ou, melhor, importar `MIN_AUTO_BACKUP_PASSWORD_LEN` nos dois caminhos, para que não voltem a divergir — e acrescentar uma checagem de qualidade além do comprimento.

### Critérios de aceite

- [ ] Existe uma única constante de mínimo compartilhada pelos dois caminhos de backup
- [ ] `POST /api/backup` com senha de 8 a 11 caracteres retorna 400
- [ ] Teste Rust cobrindo a rejeição no limite (11 recusada, 12 aceita)
- [ ] A mensagem de erro do servidor bate com a exibida na tela de Configurações

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F4, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — os dois caminhos de backup compartilham a constante de 12 caracteres.

--- FIM ISSUE 4 ---

--- ISSUE 5 ---

## [Segurança] Chave de API Firebase hardcoded no histórico do git

**Labels:** `security, media, secrets`
**Severidade:** Média
**Categoria:** 4. Chaves expostas

### Problema

É o padrão exato de "default público que vira segredo real quando a variável não é sobrescrita": o `||` garante que a chave literal entra no bundle sempre que `VITE_FIREBASE_API_KEY` não estiver definida. O arquivo não existe mais no HEAD — a autenticação migrou para local, em Rust — mas `git log -S` confirma os dois commits e `git merge-base --is-ancestor` confirma que ambos são alcançáveis a partir de `main`: qualquer clone do repositório público os traz junto. Uma chave de API Web do Firebase é, por projeto, um identificador destinado ao cliente e não uma credencial de acesso — é por isso que este achado é médio e não crítico. Ainda assim ela expõe o projeto `atendemente-432ab` a abuso da API Identity Toolkit (criação de contas, consumo de cota) e a leitura de Firestore/Storage `na medida em que as regras de segurança daquele projeto forem permissivas` — o que não é verificável a partir deste repositório.

### Evidência

- `src/lib/auth.ts — commit ad4260f:1-6`
- `src/lib/auth.ts — commit ac6b0c7:1-6`

```
// src/lib/auth.ts:1-6 — em ad4260f e ac6b0c7, ambos ancestrais de main
const apiKey = import.meta.env.VITE_FIREBASE_API_KEY || "AIzaSyCMGKi2aHlRpOynxpjhb_7VZMMMqRaUa4Y";
const authDomain = import.meta.env.VITE_FIREBASE_AUTH_DOMAIN || "atendemente-432ab.firebaseapp.com";
const projectId = import.meta.env.VITE_FIREBASE_PROJECT_ID || "atendemente-432ab";
const storageBucket = import.meta.env.VITE_FIREBASE_STORAGE_BUCKET || "atendemente-432ab.firebasestorage.app";
const messagingSenderId = import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID || "416468948741";
const appId = import.meta.env.VITE_FIREBASE_APP_ID || "1:416468948741:web:f08ce79a49801f1fc3496c";
```

### Impacto

Superfície residual num projeto Firebase que a aplicação atual não usa mais. Se o projeto seguir ativo com regras abertas, permite acesso não autenticado aos recursos dele.

**Condições de exploração:** A exploração depende inteiramente das regras de segurança do projeto Firebase e de ele continuar ativo. A presença do segredo no histórico é fato verificado.

### Correção sugerida

Verificar no console do Firebase se o projeto `atendemente-432ab` ainda existe. Se sim: revogar/rotacionar a chave, restringi-la por aplicativo e revisar as regras de Firestore e Storage; se o projeto não é mais usado, desativá-lo. Reescrever o histórico com `git filter-repo` é possível, mas num repositório público já clonado o valor deve ser tratado como comprometido de qualquer forma — desativar o projeto é a correção que de fato fecha o risco.

### Critérios de aceite

- [ ] Confirmado o estado do projeto `atendemente-432ab` (ativo ou desativado)
- [ ] Se ativo: chave rotacionada e restrita, regras de Firestore/Storage revisadas e documentadas
- [ ] Nenhum novo default literal de credencial no padrão `env || "valor"` no código-fonte (verificação em CI)
- [ ] Decisão sobre reescrita de histórico registrada, com justificativa

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F5, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Resolvido pelo usuario — o projeto Firebase `atendemente-432ab` foi excluido, que era a correcao que de fato fechava o risco.

--- FIM ISSUE 5 ---

--- ISSUE 6 ---

## [Segurança] CSP da webview permite script-src 'unsafe-inline'

**Labels:** `security, media, csp`
**Severidade:** Média
**Categoria:** 5. Inputs sem tratamento (XSS)

### Problema

O comentário em middleware.rs:25-28 registra corretamente que "a webview honra a CSP da configuração" — e é exatamente por isso que a política válida no produto instalado é a mais fraca das duas. Com `'unsafe-inline'` em `script-src`, qualquer injeção de HTML que venha a existir executa; sem ele, a mesma injeção seria inerte. A CSP da webview também não fecha `object-src`, `base-uri` nem `form-action`, que a do servidor fecha, e o teste `csp_locks_down_the_dangerous_directives` (middleware.rs:84-104) valida apenas a política do servidor — a divergência não é detectada por nenhum teste. Registre-se: `não foi encontrado nenhum sink de XSS no frontend` (zero ocorrências de innerHTML, dangerouslySetInnerHTML, eval ou new Function), de modo que isto é o enfraquecimento de uma defesa em profundidade, não uma injeção explorável hoje. O que a torna relevante é o conteúdo renderizado: nome, anotações, histórico clínico e medicações são texto livre digitado pelo usuário ou importado de CSV.

### Evidência

- `src-tauri/tauri.conf.json:25`
- `src-tauri/src/middleware.rs:29-43`
- `src-tauri/src/middleware.rs:84-104`

```
// src-tauri/tauri.conf.json:25 — a política que a webview do Tauri aplica
"csp": "default-src 'self'; connect-src 'self' http://localhost:* http://127.0.0.1:* ipc: asset: ws:;
        style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
        font-src 'self' https://fonts.gstatic.com; img-src 'self' data: blob:;
        script-src 'self' 'unsafe-inline' asset:"
                        ^^^^^^^^^^^^^^^^

// src-tauri/src/middleware.rs:37 — a política que o servidor envia
             script-src 'self'; \
             object-src 'none'; \
             base-uri 'none'; \
             form-action 'none'; \
```

### Impacto

A camada que conteria um XSS futuro está desativada precisamente no artefato distribuído aos usuários. Também não há nenhuma biblioteca de sanitização no projeto, caso um sink venha a ser introduzido.

**Condições de exploração:** Enfraquecimento de defesa em profundidade: só produz impacto direto se um sink de HTML for introduzido no frontend.

### Correção sugerida

Remover `'unsafe-inline'` de `script-src` no tauri.conf.json e alinhar as duas políticas, acrescentando `object-src 'none'`, `base-uri 'none'` e `form-action 'none'`. Estender o teste de middleware.rs para ler o tauri.conf.json e exigir das duas políticas o mesmo conjunto mínimo de diretivas, para que não voltem a divergir.

### Critérios de aceite

- [ ] `script-src` em `tauri.conf.json` não contém `'unsafe-inline'`
- [ ] `object-src 'none'`, `base-uri 'none'` e `form-action 'none'` presentes nas duas políticas
- [ ] Teste automatizado compara as diretivas de `tauri.conf.json` com as de `middleware.rs`
- [ ] O app abre e opera normalmente com a política endurecida (verificado no build de release)

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F6, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — `unsafe-inline` removido de `script-src` no tauri.conf.json, com teste que compara as duas politicas.

--- FIM ISSUE 6 ---

--- ISSUE 7 ---

## [Segurança] /auth/recover permite enumeração de contas antes do rate limit

**Labels:** `security, baixa`
**Severidade:** Baixa
**Categoria:** 2. Regra definida no navegador

### Problema

O limite de 3 tentativas por 15 minutos é aplicado sobre o `user_id`, que só existe depois de a conta ter sido encontrada. Consultas com e-mail inexistente, portanto, nunca incrementam contador nenhum: dá para varrer endereços indefinidamente e distinguir o 404 ("Nenhuma conta encontrada com este email.") do 401 ("Chave de recuperação inválida."). O contraste com o resto do arquivo é o que torna isto um achado e não uma escolha: o `login` foi cuidadosamente protegido contra o mesmo problema, gastando um Argon2 sobre um hash dummy quando a conta não existe (auth_service.rs:16-23 e 243-247) e devolvendo sempre "Email ou senha inválidos.". O caminho de recuperação não recebeu o mesmo tratamento. O servidor escuta apenas em loopback e o CORS por predicado impede que um site qualquer leia a resposta, o que mantém a severidade baixa.

### Evidência

- `src-tauri/src/auth/mod.rs:255-268`
- `src-tauri/src/auth/auth_service.rs:357-365`

```
// auth/mod.rs:255-268 — o lookup por e-mail roda ANTES do enforce_rate_limit
let user_id = match (&input.user_id, &input.email) {
    (Some(uid), _) if !uid.is_empty() => uid.clone(),
    (_, Some(email)) if !email.is_empty() => {
        auth_service::find_user_id_by_email(&state.auth_db, email)
            .await
            .map_err(|e| AppError::not_found(e))?      // <- o 404 distingue a conta
    }
    _ => return Err(AppError::bad_request("Informe user_id ou email.")),
};

crate::rate_limit::enforce_rate_limit(          // <- só agora
    &state.auth_db, "auth:password-reset", &user_id, 3, 900_000,
).await?;

// auth_service.rs:364
.ok_or_else(|| "Nenhuma conta encontrada com este email.".to_string())
```

### Impacto

Confirmação de quais endereços de e-mail têm conta na máquina, sem custo e sem limite. Útil como etapa preparatória de um ataque local ao código de recuperação.

**Condições de exploração:** Requer alcançar a API em 127.0.0.1:3001, isto é, código já em execução na máquina do usuário. O servidor não escuta na rede.

### Correção sugerida

Aplicar o rate limit sobre o identificador `recebido` — o e-mail informado — antes de resolvê-lo, e devolver a mesma resposta para conta existente e inexistente, no mesmo espírito do que já é feito no login.

### Critérios de aceite

- [ ] `enforce_rate_limit` é chamado antes de `find_user_id_by_email`, com o e-mail como identificador
- [ ] A resposta para e-mail inexistente é indistinguível da resposta para código de recuperação errado (mesmo status e mesma mensagem)
- [ ] Teste Rust cobrindo os dois casos e comparando as respostas

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F7, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — o rate limit passou a ser aplicado sobre o identificador recebido, antes do lookup, e a resposta e a mesma para conta inexistente e codigo errado.

--- FIM ISSUE 7 ---

--- ISSUE 8 ---

## [Segurança] Content-Type de download vem do MIME declarado, sem revalidação

**Labels:** `security, baixa`
**Severidade:** Baixa
**Categoria:** 5. Inputs sem tratamento (XSS)

### Problema

`create_upload_session` grava `input.mime_type` sem validar; quem valida o MIME contra a extensão e contra os magic bytes reais é `validate_file_content`, chamada apenas em `confirm_upload` (files.rs:292). Mas `download_file` não exige confirmação — a única condição é `deleted_at IS NULL`. Uma sessão de upload criada e escrita, porém nunca confirmada, fica baixável com um `Content-Type` arbitrário escolhido pelo chamador. Três coisas seguram o impacto, e por isso a severidade é baixa: o `Content-Disposition: attachment` na linha seguinte impede a renderização inline; a extensão já foi restrita a pdf/docx/png/jpg/jpeg em `validate_file_metadata` (files.rs:46-57); e o app é monousuário, de modo que atacante e vítima são a mesma conta. O defeito real é de consistência: o modelo do código presume que todo registro legível passou pela validação, e há um caminho em que isso não vale.

### Evidência

- `src-tauri/src/api/routes.rs:602`
- `src-tauri/src/features/files.rs:166-183`
- `src-tauri/src/features/files.rs:292`
- `src-tauri/src/features/files.rs:320-328`

```
// features/files.rs:179 — o MIME entra no banco como o cliente declarou
    .bind(&input.mime_type)

// features/files.rs:292 — a validação de magic bytes acontece só no confirm_upload
    validate_file_content(&data, &file.original_name, &file.mime_type)?;

// features/files.rs:320-323 — o download não revalida; exige apenas deleted_at IS NULL
    r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#

// api/routes.rs:602 — e o valor vai direto para o cabeçalho
    (HeaderName::from_static("content-type"), file.mime_type),
```

### Impacto

Um registro de arquivo não confirmado é servido com cabeçalho controlado pelo chamador. Sem impacto direto hoje, graças ao `attachment` e à whitelist de extensões.

**Condições de exploração:** Requer uma sessão autenticada da própria conta e um upload deixado deliberadamente sem confirmar.

### Correção sugerida

Marcar a confirmação no banco — uma coluna `confirmed_at` — e exigi-la em `download_file` e `list_files_by_appointment`. Alternativa mais simples: ignorar o MIME armazenado na resposta e derivar o `Content-Type` dos magic bytes do conteúdo decifrado, no momento do download.

### Critérios de aceite

- [ ] Arquivo não confirmado não é baixável (ou o `Content-Type` é derivado do conteúdo real)
- [ ] `list_files_by_appointment` não lista uploads não confirmados
- [ ] Teste Rust cobrindo o download de um registro criado mas não confirmado

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F8, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — o `Content-Type` do download e derivado dos magic bytes do conteudo decifrado.

--- FIM ISSUE 8 ---

--- ISSUE 9 ---

## [Segurança] Sem validação de tamanho de campo no servidor (crate validator não usada)

**Labels:** `security, baixa`
**Severidade:** Baixa
**Categoria:** 2. Regra definida no navegador

### Problema

`AppError` tem uma variante `Validation` pronta para `ValidationErrors` (errors.rs:148-165) e a crate está no Cargo.toml, mas nenhum struct de entrada carrega atributos `#[validate]` — a validação de forma existe só em Zod, no cliente. Na prática, um `POST /api/patients` direto aceita um `admin_notes` de vários megabytes, limitado apenas pelo `DefaultBodyLimit` global de 20 MB (lib.rs:371). As regras que `de fato` importam para segurança estão no servidor e foram verificadas: nome com no mínimo 3 caracteres (patients.rs:620), senha com no mínimo 8 (auth_service.rs:121), fim depois do início (appointments.rs:225), recorrência limitada a 52 ocorrências (appointments_recurrence.rs:18-21). O que falta é o teto de tamanho.

### Evidência

- `src-tauri/src/db/models.rs:1-375`
- `src-tauri/Cargo.toml:56`
- `src/lib/schemas.ts:14-24`

```
# src-tauri/Cargo.toml:56 — a dependência existe
validator = { version = "0.19", features = ["derive"] }

// src-tauri/src/db/models.rs — nenhuma ocorrência de #[validate] em todo o arquivo
pub struct CreatePatientInput {
    pub full_name: String,
    pub health_history: Option<String>,     // sem limite
    pub medications_in_use: Option<String>, // sem limite
    pub admin_notes: Option<String>,        // sem limite

// src/lib/schemas.ts:14-24 — a única validação de forma é a do cliente
export const patientSchema = z.object({
  full_name: z.string().min(2, "Nome deve ter no mínimo 2 caracteres"),
  health_history: z.string().optional(),
```

### Impacto

Crescimento descontrolado do banco e da memória por requisição, a partir de chamadas fora da interface. Sem impacto sobre confidencialidade ou integridade.

**Condições de exploração:** Requer chamada à API fora da interface. O limite global de corpo de 20 MB continua valendo.

### Correção sugerida

Aplicar `#[validate(length(max = ...))]` nos structs de entrada de `models.rs` e chamar `input.validate()?` no início de cada handler que os recebe — a variante de erro já está implementada e formatada.

### Critérios de aceite

- [ ] Structs de entrada em `models.rs` têm limites de comprimento declarados
- [ ] Handlers chamam `validate()` antes de persistir
- [ ] `POST /api/patients` com campo acima do limite retorna 400 com o corpo de erro de validação

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F9, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — os sete structs de entrada ganharam `#[validate]` e os handlers chamam `validate()`.

--- FIM ISSUE 9 ---

--- ISSUE 10 ---

## [Segurança] App carrega Google Fonts da internet e a CSP foi afrouxada para isso

**Labels:** `security, baixa, privacidade`
**Severidade:** Baixa
**Categoria:** 5. Inputs sem tratamento (XSS)

### Problema

Um aplicativo desktop de prontuário psicológico, que o README descreve como local, faz uma requisição a servidores do Google a cada abertura. O conteúdo do prontuário não vaza, mas o padrão de uso sim: endereço IP do consultório e horário de cada sessão de trabalho ficam registrados num terceiro que não é operador contratado para esse tratamento. É também a razão de a CSP da webview precisar liberar dois domínios externos (F6), e uma dependência de rede num app que deveria funcionar sem ela: sem internet, a tipografia cai para a fonte de sistema.

### Evidência

- `src/style.css:1`
- `src-tauri/tauri.conf.json:25`

```
/* src/style.css:1 */
@import url("https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&
             family=Newsreader:ital,wght@0,400;0,500;0,600;0,700;1,400&display=swap");

// src-tauri/tauri.conf.json:25 — a CSP precisou ser aberta por causa disso
"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
 font-src 'self' https://fonts.gstatic.com;"
```

### Impacto

Vazamento de metadados de uso para terceiro e dependência de rede desnecessária. Relevante para o registro de operadores exigido pela LGPD.

**Condições de exploração:** Ocorre em toda execução com rede disponível. Não requer configuração especial.

### Correção sugerida

Baixar os arquivos `.woff2` das duas famílias, servi-los do próprio bundle com `@font-face` local, e então remover `fonts.googleapis.com` e `fonts.gstatic.com` da CSP.

### Critérios de aceite

- [ ] As fontes são carregadas de arquivos locais do bundle
- [ ] `src/style.css` não contém `@import` de host externo
- [ ] A CSP de `tauri.conf.json` não lista `fonts.googleapis.com` nem `fonts.gstatic.com`
- [ ] O app abre sem rede e renderiza com a tipografia correta

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F10, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — as fontes sao servidas do bundle e os dois dominios sairam da CSP.

--- FIM ISSUE 10 ---

--- ISSUE 11 ---

## [Segurança] Rejeitar peppers degenerados na inicialização

**Labels:** `security, informativa`
**Severidade:** Informativa
**Categoria:** 4. Chaves expostas

### Problema

Registrado por completude da varredura, não como falha. Nenhum desses valores é segredo de produção: o pepper zerado do CI roda contra bancos em `mktemp -d` e existe justamente para que o smoke test não dependa do cofre de credenciais do runner; as senhas de e2e criam contas descartáveis. O único ponto que merece registro é que `load_or_generate_pepper` (config.rs:146-180) aceita um pepper de 32 bytes nulos sem qualquer objeção — não existe validação de inicialização que recuse valores notoriamente fracos ou de exemplo. Vale também anotar o acerto correspondente: a variável de ambiente `nunca` é persistida no cofre (o comentário em config.rs:147-154 documenta a regressão que motivou a mudança), então um pepper de teste não pode mais substituir silenciosamente o real.

### Evidência

- `.github/workflows/ci.yml:102`
- `.github/workflows/ci.yml:136`
- `e2e/fixtures/auth.ts:21`
- `e2e/fixtures/test-data.ts:89`
- `scripts/seed-demo.ts — commit a05f804:25`

```
# .github/workflows/ci.yml:102
export MASTER_PEPPER=0000000000000000000000000000000000000000000000000000000000000000

# .github/workflows/ci.yml:136
-d '{"email":"smoke@test.local","password":"senha123456","full_name":"Smoke"}'

// e2e/fixtures/auth.ts:21
const password = "test123456";

// scripts/seed-demo.ts:25 (arquivo já removido; permanece no commit a05f804)
const demoPassword = "AtendeMente123!";
```

### Impacto

Nenhum sobre a instalação do usuário final.

**Condições de exploração:** Não aplicável.

### Correção sugerida

Opcionalmente, recusar peppers degenerados — todos os bytes iguais — em `load_or_generate_pepper` quando o processo não estiver num modo de teste declarado, para que um valor de exemplo copiado da documentação não vire chave real.

### Critérios de aceite

- [ ] `load_or_generate_pepper` recusa um pepper com todos os bytes iguais fora de modo de teste
- [ ] Teste Rust cobrindo a recusa

---
_Aberto a partir do relatório de auditoria de segurança de 31 de agosto de 2026 (achado F11, versão 1.0.19, commit 6ed2d6a)._

> **Situação:** Corrigido — peppers degenerados sao recusados no boot e o `hex_decode` do config nao entra mais em panico com entrada multi-byte.

--- FIM ISSUE 11 ---

