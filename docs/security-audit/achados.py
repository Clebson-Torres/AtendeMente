# -*- coding: utf-8 -*-
"""
Dados da auditoria de segurança do AtendeMente.

Separado do gerador para que o relatório possa ser reemitido depois de uma
correção sem mexer no código de layout: edite os achados aqui e rode
`gerar_relatorio.py` de novo.
"""

PROJETO = "AtendeMente"
VERSAO_AUDITADA = "1.0.19"
COMMIT = "6ed2d6a"
DATA = "31 de agosto de 2026"

PALETA = {
    "critica": "#B91C1C",
    "alta": "#EA580C",
    "media": "#D97706",
    "baixa": "#2563EB",
    "informativa": "#64748B",
    "forte": "#059669",
}

ROTULO_SEV = {
    "critica": "Crítica",
    "alta": "Alta",
    "media": "Média",
    "baixa": "Baixa",
    "informativa": "Informativa",
}

# Mapa categoria -> rotulo curto do eixo do grafico de barras. A ordem de
# insercao define a ordem das barras.
ROTULO_CATEGORIA = {
    "1. Banco sem tranca (isolamento por usuário)": "1. Isolamento\npor usuário",
    "2. Regra definida no navegador": "2. Regra no\nnavegador",
    "3. IDOR (objeto acessado sem verificar o dono)": "3. IDOR",
    "4. Chaves expostas": "4. Chaves\nexpostas",
    "5. Inputs sem tratamento (XSS)": "5. XSS /\ninputs",
}

STACK = [
    ("Desktop / shell", "Tauri v2 (Rust), janela única, capabilities restritas a core / shell-open / dialog"),
    ("Frontend", "React 18 + TypeScript + Vite 6 + Tailwind 3, validação com Zod 4"),
    ("Backend", "Rust + Axum 0.8 embarcado no processo Tauri (também compilável como <b>bin/server</b>)"),
    ("Persistência", "SQLite via SQLx 0.8 — um banco de autenticação e um banco por usuário"),
    ("Autenticação", "Local: Argon2id + token de sessão UUIDv4 guardado como SHA-256, TTL de 7 dias"),
    ("Criptografia", "AES-256-GCM com chave por usuário derivada por HKDF-SHA256 de um pepper mestre"),
    ("Cofre de chaves", "keyring v3 (Credential Manager / Keychain / Secret Service)"),
    ("Deploy / CI", "GitHub Actions (quality + build + release). Não há Docker, Helm nem Terraform"),
]

METODOLOGIA = [
    ("1. Banco sem tranca",
     "O projeto não usa Supabase, logo não há RLS. O mecanismo de isolamento equivalente foi "
     "identificado como duplo: (a) um arquivo SQLite por usuário, em "
     "<b>&lt;DATA_DIR&gt;/&lt;user_id&gt;/atendemente.db</b> (config.rs:321), e (b) uma coluna "
     "<b>user_id</b> presente em toda tabela de dados e exigida em toda cláusula WHERE. "
     "A auditoria extraiu <b>todos</b> os literais SQL do crate e classificou cada um pela presença de "
     "escopo de dono, revisando manualmente os que não o tinham."),
    ("2. Permissão definida no navegador",
     "O app é monousuário por instalação: não existem papéis (admin/editor). A categoria foi remapeada "
     "para <b>divergência de regra entre cliente e servidor</b> — toda validação ou gate feito em React "
     "foi cruzado com o handler Axum correspondente, para verificar se o servidor reimpõe a mesma regra."),
    ("3. IDOR",
     "Percorridos os <b>34</b> registros de rota de <b>api/routes.rs</b>, as <b>9</b> rotas de "
     "<b>auth/mod.rs</b> e os <b>6</b> comandos Tauri de <b>commands.rs</b> e <b>main.rs</b> — "
     "handler por handler, não por amostragem — seguindo cada identificador recebido por path, query ou "
     "body até a consulta SQL que o consome."),
    ("4. Chaves expostas",
     "Varredura do código-fonte, de <b>tauri.conf.json</b>, <b>Cargo.toml</b>, <b>package.json</b>, "
     "workflows do GitHub Actions, scripts e documentação. O <b>histórico completo do git</b> foi varrido "
     "com <b>git log -S</b> por padrões de segredo, e verificou-se com <b>git merge-base</b> a "
     "alcançabilidade de cada commit encontrado a partir de <b>main</b>."),
    ("5. Inputs sem tratamento (XSS)",
     "Busca por sinks no frontend (innerHTML, dangerouslySetInnerHTML, eval, new Function, "
     "insertAdjacentHTML, document.write), por URLs de origem no usuário em href/src, e no backend por "
     "entrada do usuário chegando a HTML, cabeçalhos HTTP ou CSV sem escape. Como o produto é uma "
     "webview, as <b>duas</b> políticas de CSP do projeto — a do servidor e a da configuração do Tauri — "
     "foram comparadas entre si."),
]

# ---------------------------------------------------------------------------

ACHADOS = [
    dict(
        id="F1",
        status="Corrigido — migration 15 substitui os valores em claro por HMAC-SHA256 de n-gramas, com chave derivada do pepper, e a tabela ganhou `user_id`.",
        titulo="Telefone e e-mail dos pacientes ficam em texto claro no índice de busca, numa tabela "
               "que é a única sem escopo de dono",
        categoria="1. Banco sem tranca (isolamento por usuário)",
        severidade="alta",
        locais=[
            ("src-tauri/src/db/migrations/20240101000009_search_tokens.sql", "1-5"),
            ("src-tauri/src/features/patients.rs", "236-252"),
            ("src-tauri/src/features/patients.rs", "257-277"),
        ],
        trecho="""-- migrations/20240101000009_search_tokens.sql:1-5
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
    tokens.push((patient_id.to_string(), "email".to_string(), lower.clone()));""",
        porque="A migration 20240101000008 moveu a PII do paciente para um blob AES-256-GCM "
               "(<b>pii_encrypted</b>) e as colunas em claro passaram a ser gravadas como NULL. "
               "A migration 9 devolveu telefone e e-mail ao disco em texto claro, agora em outra tabela, "
               "para permitir busca por substring. O resultado é que os dois campos de contato mais "
               "identificáveis do prontuário estão legíveis para quem abrir o arquivo .db com qualquer "
               "cliente SQLite — sem senha, sem o pepper, sem passar pelo cofre de credenciais do "
               "sistema operacional. E o arquivo .db é justamente o que sai da máquina: backups em pen "
               "drive, pastas sincronizadas com a nuvem, um HD retirado de um notebook roubado. "
               "Além disso, <b>patient_search_tokens</b> é a única tabela de dados sem coluna "
               "<b>user_id</b>: o escopo de dono existe apenas indiretamente, pelo JOIN com "
               "<b>patients</b>. Hoje isso não vaza — as consultas de busca em patients.rs:360-396 "
               "prendem o resultado com um WHERE user_id externo — mas a proteção passa a depender de "
               "todo autor futuro lembrar de escrever o JOIN.",
        impacto="Exposição de dados pessoais de pacientes (LGPD, art. 46) a partir de uma cópia do "
                "arquivo de banco, anulando na prática a criptografia de PII para telefone e e-mail.",
        correcao="Guardar um token derivado irreversível em vez do valor: por exemplo HMAC-SHA256 "
                 "truncado por n-grama do telefone e do e-mail, com chave derivada do pepper. A busca "
                 "por substring passa a gerar os mesmos n-gramas a partir do termo consultado. Se a "
                 "busca em claro for considerada requisito inegociável, o mínimo é adicionar a coluna "
                 "<b>user_id</b> à tabela e ao WHERE, para que o isolamento não dependa do JOIN.",
        condicoes="Não requer nenhuma configuração especial. Explorável por qualquer um com acesso de "
                  "leitura ao arquivo <b>&lt;DATA_DIR&gt;/&lt;user_id&gt;/atendemente.db</b>.",
        aceite=[
            "`patient_search_tokens.token_text` não contém mais telefone nem e-mail legíveis "
            "(verificável com `SELECT token_text FROM patient_search_tokens` num banco de teste)",
            "A tabela tem coluna `user_id NOT NULL` e todas as consultas que a leem filtram por ela",
            "Busca por telefone e por e-mail continua funcionando (teste de integração cobrindo busca parcial)",
            "Existe migration que reindexa as linhas já gravadas em claro",
        ],
        issue_labels="security, alta, lgpd",
        issue_titulo="[Segurança] PII de contato em texto claro em patient_search_tokens",
    ),
    dict(
        id="F2",
        status="Corrigido — a entrada do cofre passou a incluir o `user_id`, as tres rotas usam o usuario autenticado e a senha legada e migrada no boot.",
        titulo="A senha dos backups automáticos vive num slot global do cofre, e as três rotas que a "
               "gerenciam descartam o usuário autenticado",
        categoria="3. IDOR (objeto acessado sem verificar o dono)",
        severidade="alta",
        locais=[
            ("src-tauri/src/config.rs", "9"),
            ("src-tauri/src/config.rs", "89-119"),
            ("src-tauri/src/api/routes.rs", "190"),
            ("src-tauri/src/api/routes.rs", "204"),
            ("src-tauri/src/api/routes.rs", "215"),
            ("src-tauri/src/lib.rs", "237"),
        ],
        trecho="""// config.rs:9 — conta fixa, sem user_id no nome
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
let Some(password) = config::load_backup_password() else { ... continue; };""",
        porque="Toda a aplicação foi construída em torno do isolamento por usuário — banco separado, "
               "chave AES separada, <b>user_id</b> em cada WHERE — e este recurso escapa dele. As três "
               "rotas de <b>/api/backup/password</b> autenticam o chamador e então jogam fora o "
               "<b>user.id</b>: o <b>_</b> em <b>let _user</b> é a assinatura da falha, porque o alvo "
               "da operação não é derivado de quem chamou. Numa instalação com mais de uma conta — o "
               "app permite registrar quantas quiser, e o próprio scheduler itera sobre "
               "<b>SELECT id FROM auth_users</b> — isso produz três abusos concretos: "
               "(a) o usuário B chama <b>PUT /api/backup/password</b> e passa a conhecer a senha que "
               "cifra os backups automáticos <b>de A</b>, bastando obter o arquivo .atendemente para "
               "ler o prontuário completo de A; (b) o usuário B chama <b>DELETE</b> e os backups "
               "automáticos de A param de ser gerados em silêncio — lib.rs:237 apenas registra um warn "
               "e segue, enquanto a tela de A continua mostrando a frequência configurada; (c) A e B "
               "não conseguem manter senhas distintas: quem gravar por último vence.",
        impacto="Quebra do isolamento entre contas para o artefato mais sensível do sistema (o backup "
                "contém o banco inteiro mais os anexos) e perda silenciosa de backup para o usuário "
                "que teve a senha removida por outro.",
        correcao="Tornar a entrada do cofre específica por usuário — por exemplo "
                 "<b>Entry::new(KEYCHAIN_SERVICE, &amp;format!(\"backup_password:{user_id}\"))</b> — e "
                 "propagar o <b>user.id</b> autenticado por <b>load/save/delete_backup_password</b> e "
                 "pelo laço do scheduler em lib.rs. Trocar <b>let _user</b> por <b>let user</b> nos "
                 "três handlers deixa o compilador cobrar o resto.",
        condicoes="Explorável quando a instalação tem mais de uma conta registrada. Com conta única o "
                  "impacto se reduz, mas o defeito de projeto permanece e vira falha assim que a "
                  "segunda conta é criada.",
        aceite=[
            "A entrada do cofre inclui o `user_id` no nome da conta",
            "`load_backup_password`, `save_backup_password` e `delete_backup_password` recebem `user_id`",
            "Os três handlers de `/api/backup/password` usam o `user.id` do token, não `_user`",
            "O scheduler em `lib.rs` busca a senha do usuário da iteração",
            "Teste: com duas contas, definir a senha em A não altera nem remove a de B",
        ],
        issue_labels="security, alta, idor",
        issue_titulo="[Segurança] Senha de backup automático é global e ignora o usuário autenticado",
    ),
    dict(
        id="F3",
        status="Corrigido — formato de backup v3: o manifesto carrega um envelope da chave do usuario cifrado pela senha, e nao mais o pepper mestre. Bundles v1 e v2 continuam restaurando.",
        titulo="O pepper mestre — chave-raiz que deriva a chave AES de todos os usuários da máquina — "
               "é gravado dentro do arquivo de backup",
        categoria="4. Chaves expostas",
        severidade="alta",
        locais=[
            ("src-tauri/src/features/backup.rs", "61"),
            ("src-tauri/src/features/backup.rs", "128-137"),
            ("src-tauri/src/features/backup.rs", "148"),
            ("src-tauri/src/crypto.rs", "46-52"),
        ],
        trecho="""// features/backup.rs:128-137
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
    let hk = Hkdf::<Sha256>::new(Some(pepper), user_id.as_bytes());""",
        porque="O pepper existe justamente para <b>não</b> sair da máquina: é gerado por CSPRNG na "
               "primeira execução e guardado no cofre de credenciais do sistema operacional "
               "(config.rs:146-229). Gravá-lo no backup transfere esse segredo para um arquivo feito "
               "para ser copiado — pen drive, e-mail, pasta sincronizada. O comentário da linha 133 "
               "reconhece o risco e o considera coberto pela cifragem do ZIP, mas isso rebaixa a "
               "proteção do pepper de \"cofre do sistema operacional\" para \"a senha que o usuário "
               "escolheu\", e o servidor aceita senha de <b>8 caracteres</b> nesse ponto "
               "(backup.rs:25 — ver F4). Como <b>derive_user_key</b> é apenas "
               "<b>HKDF(pepper, user_id)</b> e o <b>user_id</b> viaja em claro no mesmo manifesto, quem "
               "quebrar a senha do backup não obtém apenas os dados daquele usuário: obtém a chave de "
               "<b>todos</b> os usuários daquela instalação, e ela continua válida para os bancos que "
               "permanecem na máquina. O motivo funcional é legítimo — restaurar num computador novo, "
               "onde o cofre não existe mais — mas a solução escolhida embute a chave-raiz no artefato.",
        impacto="O comprometimento de um único arquivo de backup, com a senha quebrada, expõe a "
                "chave-raiz de criptografia da instalação inteira — inclusive de contas cujo backup "
                "nunca vazou.",
        correcao="Não transportar o pepper. Cifrar a chave por usuário com uma chave derivada da senha "
                 "do backup (Argon2id) e guardar apenas esse envelope no manifesto, de modo que o "
                 "segredo transportado seja função da senha e não a chave-raiz da máquina. Enquanto "
                 "isso não for feito: elevar o mínimo de senha (F4), manter o "
                 "<b>pepper_fingerprint</b> — que já existe e basta para detectar divergência de chave "
                 "— e remover o campo <b>pepper</b>, passando a exigir o pepper antigo do usuário na "
                 "restauração entre máquinas.",
        condicoes="Presente em todo backup cifrado: o campo só é preenchido quando <b>is_encrypted</b>. "
                  "A exploração exige obter o arquivo e quebrar a senha escolhida pelo usuário.",
        aceite=[
            "`manifest.json` de um backup recém-criado não contém material de chave no campo `pepper`",
            "Restauração numa máquina com pepper diferente continua funcionando (teste de integração, "
            "reaproveitando as fixtures de `src-tauri/tests/fixtures/`)",
            "Backups já emitidos no formato antigo continuam restauráveis",
            "O `pepper_fingerprint` segue presente para detecção de divergência",
        ],
        issue_labels="security, alta, crypto",
        issue_titulo="[Segurança] Master pepper é serializado dentro do manifesto do backup",
    ),
    dict(
        id="F4",
        status="Corrigido — os dois caminhos de backup compartilham a constante de 12 caracteres.",
        titulo="Tamanho mínimo da senha de backup: 12 caracteres no navegador, 8 no servidor",
        categoria="2. Regra definida no navegador",
        severidade="media",
        locais=[
            ("src/pages/Settings.tsx", "55"),
            ("src/pages/Settings.tsx", "104"),
            ("src/pages/Settings.tsx", "154"),
            ("src-tauri/src/features/backup.rs", "25"),
            ("src-tauri/src/features/backup.rs", "88-95"),
            ("src-tauri/src/config.rs", "12"),
        ],
        trecho="""// src/pages/Settings.tsx:55 — o que o usuário vê
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
pub const MIN_AUTO_BACKUP_PASSWORD_LEN: usize = 12;""",
        porque="A regra que o usuário enxerga é imposta apenas em React. Um <b>POST /api/backup</b> "
               "feito fora da interface — com curl, num script de automação, ou por qualquer código "
               "que passe a chamar a API depois — produz um backup válido protegido por 8 caracteres. "
               "E esse backup é o bundle completo: banco de pacientes, prontuários de sessão, anexos "
               "e, hoje, o pepper mestre (F3). A própria base de código já discorda de si mesma: "
               "<b>MIN_AUTO_BACKUP_PASSWORD_LEN = 12</b> em config.rs para a senha dos backups "
               "automáticos, contra <b>MIN_BACKUP_PASSWORD_LEN = 8</b> para o manual. E em nenhum dos "
               "dois caminhos há qualquer exigência além do comprimento.",
        impacto="Backups contendo o prontuário completo podem ser gerados com senha muito mais fraca do "
                "que a aplicação promete, reduzindo o custo de um ataque offline sobre o arquivo — e, "
                "por tabela, sobre a chave-raiz que ele carrega.",
        correcao="Elevar <b>MIN_BACKUP_PASSWORD_LEN</b> ao mesmo valor da constante de config.rs — ou, "
                 "melhor, importar <b>MIN_AUTO_BACKUP_PASSWORD_LEN</b> nos dois caminhos, para que não "
                 "voltem a divergir — e acrescentar uma checagem de qualidade além do comprimento.",
        condicoes="Explorável por qualquer chamada à API fora da interface. Não requer configuração "
                  "insegura.",
        aceite=[
            "Existe uma única constante de mínimo compartilhada pelos dois caminhos de backup",
            "`POST /api/backup` com senha de 8 a 11 caracteres retorna 400",
            "Teste Rust cobrindo a rejeição no limite (11 recusada, 12 aceita)",
            "A mensagem de erro do servidor bate com a exibida na tela de Configurações",
        ],
        issue_labels="security, media",
        issue_titulo="[Segurança] Mínimo de senha de backup diverge entre cliente (12) e servidor (8)",
    ),
    dict(
        id="F5",
        status="Resolvido pelo usuario — o projeto Firebase `atendemente-432ab` foi excluido, que era a correcao que de fato fechava o risco.",
        titulo="Chave de API e identificadores de projeto Firebase permanecem no histórico do git",
        categoria="4. Chaves expostas",
        severidade="media",
        locais=[
            ("src/lib/auth.ts — commit ad4260f", "1-6"),
            ("src/lib/auth.ts — commit ac6b0c7", "1-6"),
        ],
        trecho="""// src/lib/auth.ts:1-6 — em ad4260f e ac6b0c7, ambos ancestrais de main
const apiKey = import.meta.env.VITE_FIREBASE_API_KEY || "AIzaSyCMGKi2aHlRpOynxpjhb_7VZMMMqRaUa4Y";
const authDomain = import.meta.env.VITE_FIREBASE_AUTH_DOMAIN || "atendemente-432ab.firebaseapp.com";
const projectId = import.meta.env.VITE_FIREBASE_PROJECT_ID || "atendemente-432ab";
const storageBucket = import.meta.env.VITE_FIREBASE_STORAGE_BUCKET || "atendemente-432ab.firebasestorage.app";
const messagingSenderId = import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID || "416468948741";
const appId = import.meta.env.VITE_FIREBASE_APP_ID || "1:416468948741:web:f08ce79a49801f1fc3496c";""",
        porque="É o padrão exato de \"default público que vira segredo real quando a variável não é "
               "sobrescrita\": o <b>||</b> garante que a chave literal entra no bundle sempre que "
               "<b>VITE_FIREBASE_API_KEY</b> não estiver definida. O arquivo não existe mais no HEAD — "
               "a autenticação migrou para local, em Rust — mas <b>git log -S</b> confirma os dois "
               "commits e <b>git merge-base --is-ancestor</b> confirma que ambos são alcançáveis a "
               "partir de <b>main</b>: qualquer clone do repositório público os traz junto. "
               "Uma chave de API Web do Firebase é, por projeto, um identificador destinado ao cliente "
               "e não uma credencial de acesso — é por isso que este achado é médio e não crítico. "
               "Ainda assim ela expõe o projeto <b>atendemente-432ab</b> a abuso da API Identity "
               "Toolkit (criação de contas, consumo de cota) e a leitura de Firestore/Storage <b>na "
               "medida em que as regras de segurança daquele projeto forem permissivas</b> — o que não "
               "é verificável a partir deste repositório.",
        impacto="Superfície residual num projeto Firebase que a aplicação atual não usa mais. Se o "
                "projeto seguir ativo com regras abertas, permite acesso não autenticado aos recursos "
                "dele.",
        correcao="Verificar no console do Firebase se o projeto <b>atendemente-432ab</b> ainda existe. "
                 "Se sim: revogar/rotacionar a chave, restringi-la por aplicativo e revisar as regras "
                 "de Firestore e Storage; se o projeto não é mais usado, desativá-lo. Reescrever o "
                 "histórico com <b>git filter-repo</b> é possível, mas num repositório público já "
                 "clonado o valor deve ser tratado como comprometido de qualquer forma — desativar o "
                 "projeto é a correção que de fato fecha o risco.",
        condicoes="A exploração depende inteiramente das regras de segurança do projeto Firebase e de "
                  "ele continuar ativo. A presença do segredo no histórico é fato verificado.",
        aceite=[
            "Confirmado o estado do projeto `atendemente-432ab` (ativo ou desativado)",
            "Se ativo: chave rotacionada e restrita, regras de Firestore/Storage revisadas e documentadas",
            "Nenhum novo default literal de credencial no padrão `env || \"valor\"` no código-fonte "
            "(verificação em CI)",
            "Decisão sobre reescrita de histórico registrada, com justificativa",
        ],
        issue_labels="security, media, secrets",
        issue_titulo="[Segurança] Chave de API Firebase hardcoded no histórico do git",
    ),
    dict(
        id="F6",
        status="Corrigido — `unsafe-inline` removido de `script-src` no tauri.conf.json, com teste que compara as duas politicas.",
        titulo="A CSP que vale para a webview permite script-src 'unsafe-inline' — a do servidor, mais "
               "estrita, não é a que protege o app instalado",
        categoria="5. Inputs sem tratamento (XSS)",
        severidade="media",
        locais=[
            ("src-tauri/tauri.conf.json", "25"),
            ("src-tauri/src/middleware.rs", "29-43"),
            ("src-tauri/src/middleware.rs", "84-104"),
        ],
        trecho="""// src-tauri/tauri.conf.json:25 — a política que a webview do Tauri aplica
"csp": "default-src 'self'; connect-src 'self' http://localhost:* http://127.0.0.1:* ipc: asset: ws:;
        style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
        font-src 'self' https://fonts.gstatic.com; img-src 'self' data: blob:;
        script-src 'self' 'unsafe-inline' asset:"
                        ^^^^^^^^^^^^^^^^

// src-tauri/src/middleware.rs:37 — a política que o servidor envia
             script-src 'self'; \\
             object-src 'none'; \\
             base-uri 'none'; \\
             form-action 'none'; \\""",
        porque="O comentário em middleware.rs:25-28 registra corretamente que \"a webview honra a CSP "
               "da configuração\" — e é exatamente por isso que a política válida no produto instalado "
               "é a mais fraca das duas. Com <b>'unsafe-inline'</b> em <b>script-src</b>, qualquer "
               "injeção de HTML que venha a existir executa; sem ele, a mesma injeção seria inerte. "
               "A CSP da webview também não fecha <b>object-src</b>, <b>base-uri</b> nem "
               "<b>form-action</b>, que a do servidor fecha, e o teste "
               "<b>csp_locks_down_the_dangerous_directives</b> (middleware.rs:84-104) valida apenas a "
               "política do servidor — a divergência não é detectada por nenhum teste. "
               "Registre-se: <b>não foi encontrado nenhum sink de XSS no frontend</b> (zero ocorrências "
               "de innerHTML, dangerouslySetInnerHTML, eval ou new Function), de modo que isto é o "
               "enfraquecimento de uma defesa em profundidade, não uma injeção explorável hoje. "
               "O que a torna relevante é o conteúdo renderizado: nome, anotações, histórico clínico e "
               "medicações são texto livre digitado pelo usuário ou importado de CSV.",
        impacto="A camada que conteria um XSS futuro está desativada precisamente no artefato "
                "distribuído aos usuários. Também não há nenhuma biblioteca de sanitização no projeto, "
                "caso um sink venha a ser introduzido.",
        correcao="Remover <b>'unsafe-inline'</b> de <b>script-src</b> no tauri.conf.json e alinhar as "
                 "duas políticas, acrescentando <b>object-src 'none'</b>, <b>base-uri 'none'</b> e "
                 "<b>form-action 'none'</b>. Estender o teste de middleware.rs para ler o "
                 "tauri.conf.json e exigir das duas políticas o mesmo conjunto mínimo de diretivas, "
                 "para que não voltem a divergir.",
        condicoes="Enfraquecimento de defesa em profundidade: só produz impacto direto se um sink de "
                  "HTML for introduzido no frontend.",
        aceite=[
            "`script-src` em `tauri.conf.json` não contém `'unsafe-inline'`",
            "`object-src 'none'`, `base-uri 'none'` e `form-action 'none'` presentes nas duas políticas",
            "Teste automatizado compara as diretivas de `tauri.conf.json` com as de `middleware.rs`",
            "O app abre e opera normalmente com a política endurecida (verificado no build de release)",
        ],
        issue_labels="security, media, csp",
        issue_titulo="[Segurança] CSP da webview permite script-src 'unsafe-inline'",
    ),
    dict(
        id="F7",
        status="Corrigido — o rate limit passou a ser aplicado sobre o identificador recebido, antes do lookup, e a resposta e a mesma para conta inexistente e codigo errado.",
        titulo="Enumeração de contas em /auth/recover: a busca por e-mail acontece antes do rate limit "
               "e a mensagem distingue conta existente de inexistente",
        categoria="2. Regra definida no navegador",
        severidade="baixa",
        locais=[
            ("src-tauri/src/auth/mod.rs", "255-268"),
            ("src-tauri/src/auth/auth_service.rs", "357-365"),
        ],
        trecho="""// auth/mod.rs:255-268 — o lookup por e-mail roda ANTES do enforce_rate_limit
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
.ok_or_else(|| "Nenhuma conta encontrada com este email.".to_string())""",
        porque="O limite de 3 tentativas por 15 minutos é aplicado sobre o <b>user_id</b>, que só "
               "existe depois de a conta ter sido encontrada. Consultas com e-mail inexistente, "
               "portanto, nunca incrementam contador nenhum: dá para varrer endereços indefinidamente "
               "e distinguir o 404 (\"Nenhuma conta encontrada com este email.\") do 401 (\"Chave de "
               "recuperação inválida.\"). O contraste com o resto do arquivo é o que torna isto um "
               "achado e não uma escolha: o <b>login</b> foi cuidadosamente protegido contra o mesmo "
               "problema, gastando um Argon2 sobre um hash dummy quando a conta não existe "
               "(auth_service.rs:16-23 e 243-247) e devolvendo sempre \"Email ou senha inválidos.\". "
               "O caminho de recuperação não recebeu o mesmo tratamento. O servidor escuta apenas em "
               "loopback e o CORS por predicado impede que um site qualquer leia a resposta, o que "
               "mantém a severidade baixa.",
        impacto="Confirmação de quais endereços de e-mail têm conta na máquina, sem custo e sem limite. "
                "Útil como etapa preparatória de um ataque local ao código de recuperação.",
        correcao="Aplicar o rate limit sobre o identificador <b>recebido</b> — o e-mail informado — "
                 "antes de resolvê-lo, e devolver a mesma resposta para conta existente e inexistente, "
                 "no mesmo espírito do que já é feito no login.",
        condicoes="Requer alcançar a API em 127.0.0.1:3001, isto é, código já em execução na máquina "
                  "do usuário. O servidor não escuta na rede.",
        aceite=[
            "`enforce_rate_limit` é chamado antes de `find_user_id_by_email`, com o e-mail como identificador",
            "A resposta para e-mail inexistente é indistinguível da resposta para código de recuperação "
            "errado (mesmo status e mesma mensagem)",
            "Teste Rust cobrindo os dois casos e comparando as respostas",
        ],
        issue_labels="security, baixa",
        issue_titulo="[Segurança] /auth/recover permite enumeração de contas antes do rate limit",
    ),
    dict(
        id="F8",
        status="Corrigido — o `Content-Type` do download e derivado dos magic bytes do conteudo decifrado.",
        titulo="O Content-Type do download vem do MIME declarado no upload, que só é validado se o "
               "upload for confirmado",
        categoria="5. Inputs sem tratamento (XSS)",
        severidade="baixa",
        locais=[
            ("src-tauri/src/api/routes.rs", "602"),
            ("src-tauri/src/features/files.rs", "166-183"),
            ("src-tauri/src/features/files.rs", "292"),
            ("src-tauri/src/features/files.rs", "320-328"),
        ],
        trecho="""// features/files.rs:179 — o MIME entra no banco como o cliente declarou
    .bind(&input.mime_type)

// features/files.rs:292 — a validação de magic bytes acontece só no confirm_upload
    validate_file_content(&data, &file.original_name, &file.mime_type)?;

// features/files.rs:320-323 — o download não revalida; exige apenas deleted_at IS NULL
    r#"SELECT * FROM record_files WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#

// api/routes.rs:602 — e o valor vai direto para o cabeçalho
    (HeaderName::from_static("content-type"), file.mime_type),""",
        porque="<b>create_upload_session</b> grava <b>input.mime_type</b> sem validar; quem valida o "
               "MIME contra a extensão e contra os magic bytes reais é <b>validate_file_content</b>, "
               "chamada apenas em <b>confirm_upload</b> (files.rs:292). Mas <b>download_file</b> não "
               "exige confirmação — a única condição é <b>deleted_at IS NULL</b>. Uma sessão de upload "
               "criada e escrita, porém nunca confirmada, fica baixável com um <b>Content-Type</b> "
               "arbitrário escolhido pelo chamador. Três coisas seguram o impacto, e por isso a "
               "severidade é baixa: o <b>Content-Disposition: attachment</b> na linha seguinte impede "
               "a renderização inline; a extensão já foi restrita a pdf/docx/png/jpg/jpeg em "
               "<b>validate_file_metadata</b> (files.rs:46-57); e o app é monousuário, de modo que "
               "atacante e vítima são a mesma conta. O defeito real é de consistência: o modelo do "
               "código presume que todo registro legível passou pela validação, e há um caminho em que "
               "isso não vale.",
        impacto="Um registro de arquivo não confirmado é servido com cabeçalho controlado pelo "
                "chamador. Sem impacto direto hoje, graças ao <b>attachment</b> e à whitelist de "
                "extensões.",
        correcao="Marcar a confirmação no banco — uma coluna <b>confirmed_at</b> — e exigi-la em "
                 "<b>download_file</b> e <b>list_files_by_appointment</b>. Alternativa mais simples: "
                 "ignorar o MIME armazenado na resposta e derivar o <b>Content-Type</b> dos magic bytes "
                 "do conteúdo decifrado, no momento do download.",
        condicoes="Requer uma sessão autenticada da própria conta e um upload deixado deliberadamente "
                  "sem confirmar.",
        aceite=[
            "Arquivo não confirmado não é baixável (ou o `Content-Type` é derivado do conteúdo real)",
            "`list_files_by_appointment` não lista uploads não confirmados",
            "Teste Rust cobrindo o download de um registro criado mas não confirmado",
        ],
        issue_labels="security, baixa",
        issue_titulo="[Segurança] Content-Type de download vem do MIME declarado, sem revalidação",
    ),
    dict(
        id="F9",
        status="Corrigido — os sete structs de entrada ganharam `#[validate]` e os handlers chamam `validate()`.",
        titulo="Nenhum limite de tamanho de campo é imposto no servidor: a crate validator está "
               "declarada mas nenhum modelo a utiliza",
        categoria="2. Regra definida no navegador",
        severidade="baixa",
        locais=[
            ("src-tauri/src/db/models.rs", "1-375"),
            ("src-tauri/Cargo.toml", "56"),
            ("src/lib/schemas.ts", "14-24"),
        ],
        trecho="""# src-tauri/Cargo.toml:56 — a dependência existe
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
  health_history: z.string().optional(),""",
        porque="<b>AppError</b> tem uma variante <b>Validation</b> pronta para <b>ValidationErrors</b> "
               "(errors.rs:148-165) e a crate está no Cargo.toml, mas nenhum struct de entrada carrega "
               "atributos <b>#[validate]</b> — a validação de forma existe só em Zod, no cliente. Na "
               "prática, um <b>POST /api/patients</b> direto aceita um <b>admin_notes</b> de vários "
               "megabytes, limitado apenas pelo <b>DefaultBodyLimit</b> global de 20 MB (lib.rs:371). "
               "As regras que <b>de fato</b> importam para segurança estão no servidor e foram "
               "verificadas: nome com no mínimo 3 caracteres (patients.rs:620), senha com no mínimo 8 "
               "(auth_service.rs:121), fim depois do início (appointments.rs:225), recorrência limitada "
               "a 52 ocorrências (appointments_recurrence.rs:18-21). O que falta é o teto de tamanho.",
        impacto="Crescimento descontrolado do banco e da memória por requisição, a partir de chamadas "
                "fora da interface. Sem impacto sobre confidencialidade ou integridade.",
        correcao="Aplicar <b>#[validate(length(max = ...))]</b> nos structs de entrada de "
                 "<b>models.rs</b> e chamar <b>input.validate()?</b> no início de cada handler que os "
                 "recebe — a variante de erro já está implementada e formatada.",
        condicoes="Requer chamada à API fora da interface. O limite global de corpo de 20 MB continua "
                  "valendo.",
        aceite=[
            "Structs de entrada em `models.rs` têm limites de comprimento declarados",
            "Handlers chamam `validate()` antes de persistir",
            "`POST /api/patients` com campo acima do limite retorna 400 com o corpo de erro de validação",
        ],
        issue_labels="security, baixa",
        issue_titulo="[Segurança] Sem validação de tamanho de campo no servidor (crate validator não usada)",
    ),
    dict(
        id="F10",
        status="Corrigido — as fontes sao servidas do bundle e os dois dominios sairam da CSP.",
        titulo="O app busca fontes na internet a cada abertura, e a CSP foi afrouxada para permitir",
        categoria="5. Inputs sem tratamento (XSS)",
        severidade="baixa",
        locais=[
            ("src/style.css", "1"),
            ("src-tauri/tauri.conf.json", "25"),
        ],
        trecho="""/* src/style.css:1 */
@import url("https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&
             family=Newsreader:ital,wght@0,400;0,500;0,600;0,700;1,400&display=swap");

// src-tauri/tauri.conf.json:25 — a CSP precisou ser aberta por causa disso
"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;
 font-src 'self' https://fonts.gstatic.com;\"""",
        porque="Um aplicativo desktop de prontuário psicológico, que o README descreve como local, faz "
               "uma requisição a servidores do Google a cada abertura. O conteúdo do prontuário não "
               "vaza, mas o padrão de uso sim: endereço IP do consultório e horário de cada sessão de "
               "trabalho ficam registrados num terceiro que não é operador contratado para esse "
               "tratamento. É também a razão de a CSP da webview precisar liberar dois domínios "
               "externos (F6), e uma dependência de rede num app que deveria funcionar sem ela: sem "
               "internet, a tipografia cai para a fonte de sistema.",
        impacto="Vazamento de metadados de uso para terceiro e dependência de rede desnecessária. "
                "Relevante para o registro de operadores exigido pela LGPD.",
        correcao="Baixar os arquivos <b>.woff2</b> das duas famílias, servi-los do próprio bundle com "
                 "<b>@font-face</b> local, e então remover <b>fonts.googleapis.com</b> e "
                 "<b>fonts.gstatic.com</b> da CSP.",
        condicoes="Ocorre em toda execução com rede disponível. Não requer configuração especial.",
        aceite=[
            "As fontes são carregadas de arquivos locais do bundle",
            "`src/style.css` não contém `@import` de host externo",
            "A CSP de `tauri.conf.json` não lista `fonts.googleapis.com` nem `fonts.gstatic.com`",
            "O app abre sem rede e renderiza com a tipografia correta",
        ],
        issue_labels="security, baixa, privacidade",
        issue_titulo="[Segurança] App carrega Google Fonts da internet e a CSP foi afrouxada para isso",
    ),
    dict(
        id="F11",
        status="Corrigido — peppers degenerados sao recusados no boot e o `hex_decode` do config nao entra mais em panico com entrada multi-byte.",
        titulo="Credenciais de teste e um pepper zerado em CI, scripts e fixtures",
        categoria="4. Chaves expostas",
        severidade="informativa",
        locais=[
            (".github/workflows/ci.yml", "102"),
            (".github/workflows/ci.yml", "136"),
            ("e2e/fixtures/auth.ts", "21"),
            ("e2e/fixtures/test-data.ts", "89"),
            ("scripts/seed-demo.ts — commit a05f804", "25"),
        ],
        trecho="""# .github/workflows/ci.yml:102
export MASTER_PEPPER=0000000000000000000000000000000000000000000000000000000000000000

# .github/workflows/ci.yml:136
-d '{"email":"smoke@test.local","password":"senha123456","full_name":"Smoke"}'

// e2e/fixtures/auth.ts:21
const password = "test123456";

// scripts/seed-demo.ts:25 (arquivo já removido; permanece no commit a05f804)
const demoPassword = "AtendeMente123!";""",
        porque="Registrado por completude da varredura, não como falha. Nenhum desses valores é segredo "
               "de produção: o pepper zerado do CI roda contra bancos em <b>mktemp -d</b> e existe "
               "justamente para que o smoke test não dependa do cofre de credenciais do runner; as "
               "senhas de e2e criam contas descartáveis. O único ponto que merece registro é que "
               "<b>load_or_generate_pepper</b> (config.rs:146-180) aceita um pepper de 32 bytes nulos "
               "sem qualquer objeção — não existe validação de inicialização que recuse valores "
               "notoriamente fracos ou de exemplo. Vale também anotar o acerto correspondente: a "
               "variável de ambiente <b>nunca</b> é persistida no cofre (o comentário em "
               "config.rs:147-154 documenta a regressão que motivou a mudança), então um pepper de "
               "teste não pode mais substituir silenciosamente o real.",
        impacto="Nenhum sobre a instalação do usuário final.",
        correcao="Opcionalmente, recusar peppers degenerados — todos os bytes iguais — em "
                 "<b>load_or_generate_pepper</b> quando o processo não estiver num modo de teste "
                 "declarado, para que um valor de exemplo copiado da documentação não vire chave real.",
        condicoes="Não aplicável.",
        aceite=[
            "`load_or_generate_pepper` recusa um pepper com todos os bytes iguais fora de modo de teste",
            "Teste Rust cobrindo a recusa",
        ],
        issue_labels="security, informativa",
        issue_titulo="[Segurança] Rejeitar peppers degenerados na inicialização",
    ),
]

# ---------------------------------------------------------------------------

PONTOS_FORTES = [
    dict(
        titulo="Isolamento por usuário aplicado em 100% das consultas de dados",
        evidencia="Extraímos todos os literais SQL do crate e conferimos um a um. Cada tabela de dados "
                  "(patients, appointments, payments, session_records, record_files, recurring_series, "
                  "backup_config, audit_logs) carrega <b>user_id</b>, e toda consulta de leitura, "
                  "escrita, agregação e exportação inclui esse filtro. Exemplos: patients.rs:333, "
                  "appointments.rs:47, payments.rs:125, dashboard.rs:36-81, exports.rs:87, "
                  "files.rs:195. Isso soma-se ao isolamento físico de um arquivo SQLite por usuário "
                  "(config.rs:321-325).",
    ),
    dict(
        titulo="Os 34 handlers de dados autenticam e derivam o banco do token, nunca do cliente",
        evidencia="Nenhum handler de <b>api/routes.rs</b> aceita um <b>user_id</b> vindo do cliente. "
                  "O padrão é invariável: <b>get_authenticated_user(&amp;headers, &amp;state)</b> "
                  "(routes.rs:50-73) e em seguida <b>state.get_or_open_user_db(&amp;user.id)</b>. "
                  "Os 6 comandos Tauri seguem o mesmo caminho por <b>validate_token_and_get_db</b> "
                  "(commands.rs:22-36).",
    ),
    dict(
        titulo="Verificação de posse presente em cada operação por ID — nenhum IDOR encontrado",
        evidencia="Percorridos handler por handler. <b>update_patient</b> e <b>set_patient_status</b> "
                  "abrem com <b>get_patient_detail(db, user_id, id)</b>, que devolve 404 para objeto "
                  "alheio (patients.rs:688 e 765); <b>cancel_recurring_series</b> checa "
                  "<b>rows_affected() == 0</b> (appointments.rs:493); as oito consultas de "
                  "<b>files.rs</b> usam <b>WHERE id = ? AND user_id = ?</b>; e "
                  "<b>create_upload_session</b> valida ainda o vínculo consulta↔paciente e "
                  "pagamento↔consulta (files.rs:111-139).",
    ),
    dict(
        titulo="SQL integralmente parametrizado — nenhuma injeção possível",
        evidencia="Não há uma única interpolação de entrada do usuário em SQL. Os dois lugares que "
                  "montam SQL por formatação usam material controlado pelo próprio servidor: "
                  "<b>VACUUM INTO</b> com um caminho temporário gerado internamente e escapado "
                  "(backup.rs:108 e 835-837), e os nomes de tabela do restore, tirados da constante "
                  "<b>RESTORE_TABLE_ORDER</b> (backup.rs:465-467) — com o comentário explícito "
                  "\"Only names from this list are ever interpolated into SQL\".",
    ),
    dict(
        titulo="Frontend React sem nenhum sink de XSS",
        evidencia="Zero ocorrências de <b>innerHTML</b>, <b>dangerouslySetInnerHTML</b>, <b>eval</b>, "
                  "<b>new Function</b>, <b>insertAdjacentHTML</b> ou <b>document.write</b> em todo "
                  "<b>src/</b>. Não há renderização de markdown nem de HTML. A única URL montada com "
                  "dado do usuário é o link do WhatsApp, e ela passa por "
                  "<b>replace(/\\D/g, \"\")</b>, que descarta tudo que não for dígito e torna "
                  "<b>javascript:</b> impossível (PatientDetail.tsx:101). O link do Meet é estático e "
                  "leva <b>rel=\"noopener noreferrer\"</b> (PatientDetail.tsx:84).",
    ),
    dict(
        titulo="Upload de anexos validado em três camadas independentes",
        evidencia="Whitelist de extensões e lista de bloqueio (files.rs:12-13 e 46-57); coerência entre "
                  "MIME declarado e extensão; e conferência dos <b>magic bytes</b> reais via crate "
                  "<b>infer</b> (files.rs:59-100). O nome é normalizado contra travessia de diretório e "
                  "caracteres de controle (files.rs:15-36), o caminho de destino exige que user, "
                  "patient e appointment sejam UUIDs válidos (config.rs:334-339), e o conteúdo é "
                  "cifrado com AES-256-GCM antes de tocar o disco (files.rs:210-217).",
    ),
    dict(
        titulo="Restauração de backup fecha zip-slip e recusa bundle de outro usuário",
        evidencia="<b>safe_join</b> rejeita caminho absoluto, <b>..</b>, <b>\\</b> e <b>:</b> "
                  "(backup.rs:823-833); <b>validate_hashes</b> aplica a mesma checagem e ainda confere "
                  "o SHA-256 de cada entrada contra o manifesto (backup.rs:435-450); e "
                  "<b>manifest.user_id != user_id</b> aborta a restauração (backup.rs:252). A "
                  "restauração inteira é preparada em área de staging e só então trocada, com rollback "
                  "dos anexos em caso de falha (backup.rs:794-821).",
    ),
    dict(
        titulo="Autenticação construída com cuidado acima da média",
        evidencia="Argon2id para senha (auth_service.rs:64-79); token de sessão nunca armazenado em "
                  "claro, só o SHA-256 (auth_service.rs:104-108); comparação de tempo constante do "
                  "código de recuperação (auth_service.rs:27-37); hash dummy no login para não vazar "
                  "existência de conta por tempo de resposta (auth_service.rs:16-23 e 243-247); código "
                  "de recuperação de uso único, rotacionado no reset (auth_service.rs:463-478); e rate "
                  "limit em register (3/h), login (5/10min), recover (3/15min), reset-password "
                  "(5/15min) e unlock (5/10min).",
    ),
    dict(
        titulo="O bloqueio de tela é imposto pelo servidor, não pela interface",
        evidencia="<b>lock_handler</b> apaga a chave AES do cache do processo e fecha o pool do banco "
                  "do usuário (auth/mod.rs:335-336); o <b>unlock</b> exige senha verificada contra o "
                  "hash Argon2 e passa por rate limit (auth/mod.rs:366-383). O <b>/auth/me</b> informa "
                  "<b>locked</b> ao cliente, para evitar que a tela abra mostrando PII vazia como se "
                  "fosse dado real (auth/mod.rs:237). O logout também limpa a chave, não só o lock "
                  "(auth/mod.rs:207).",
    ),
    dict(
        titulo="Superfície de rede reduzida ao mínimo",
        evidencia="O servidor liga-se incondicionalmente a <b>127.0.0.1</b> e <b>[::1]</b> "
                  "(lib.rs:407 e 418), e a regra de firewall da antiga função de acesso móvel é "
                  "removida na primeira execução. O CORS usa predicado de origem em vez de <b>Any</b> "
                  "(lib.rs:93-129 e 375-387), com <b>allow_credentials(false)</b>. O servidor de "
                  "estáticos fecha travessia de diretório com <b>canonicalize</b> + <b>starts_with</b> "
                  "(lib.rs:314-339).",
    ),
    dict(
        titulo="Erros internos não vazam detalhe de implementação",
        evidencia="A variante <b>Internal</b> de <b>AppError</b> é substituída por uma mensagem genérica "
                  "antes de virar resposta (errors.rs:135-145), com testes dedicados "
                  "(<b>internal_errors_are_sanitized</b>, <b>database_errors_are_sanitized</b>, "
                  "<b>crypto_errors_are_sanitized</b>). Os cabeçalhos de segurança são aplicados com "
                  "<b>layer</b> e não <b>route_layer</b>, de forma a cobrir também o fallback que serve "
                  "o index.html (lib.rs:367-370).",
    ),
    dict(
        titulo="Exportação CSV protegida contra injeção de fórmula",
        evidencia="<b>escape_csv</b> prefixa com aspa simples todo campo iniciado por <b>=</b>, "
                  "<b>+</b>, <b>-</b>, <b>@</b>, tab ou CR, neutralizando a execução de fórmulas ao "
                  "abrir o arquivo em Excel ou LibreOffice (exports.rs:44-66) — um vetor que passa "
                  "despercebido na maioria das bases que exportam dado digitado pelo usuário.",
    ),
]

PONTOS_FRACOS = [
    ("A criptografia de PII tem um furo declarado",
     "Telefone e e-mail ficam legíveis no índice de busca (F1). Quem obtiver o arquivo .db obtém os "
     "dados de contato de todos os pacientes sem precisar do pepper — o que reduz bastante o valor "
     "prático do envelope AES para esses dois campos."),
    ("O backup concentra risco demais num arquivo só",
     "Ele reúne o banco completo, os anexos decifrados e a chave-raiz da instalação (F3), e o servidor "
     "aceita protegê-lo com 8 caracteres (F4). É o artefato mais exposto do sistema — feito para ser "
     "copiado — e o que carrega mais segredo."),
    ("Um recurso por usuário escapou do modelo de isolamento",
     "A senha dos backups automáticos é global e as rotas que a gerenciam ignoram quem chamou (F2). "
     "É a única exceção encontrada num modelo que, no resto, é aplicado com rigor."),
    ("A defesa contra XSS mais forte não é a que roda no produto",
     "A CSP da webview é mais permissiva que a do servidor (F6) e não há biblioteca de sanitização no "
     "projeto. Hoje não existe sink, mas a rede de proteção está desligada justamente onde o usuário "
     "final executa o app."),
    ("Regras de entrada moram no cliente",
     "Tamanho mínimo de senha de backup (F4) e limites de tamanho de campo (F9) são impostos em "
     "TypeScript e não reimpostos em Rust. Qualquer chamador fora da interface os contorna."),
]

RECOMENDACOES = [
    ("P1", "Escopar a senha de backup por usuário",
     "Incluir o <b>user_id</b> na entrada do cofre e propagá-lo pelas três rotas e pelo scheduler. É a "
     "correção de menor custo entre as de severidade alta e fecha uma quebra direta de isolamento.",
     "F2"),
    ("P1", "Parar de transportar o pepper mestre dentro do backup",
     "Substituir o campo <b>pepper</b> por um envelope da chave do usuário cifrado com a senha do "
     "backup, mantendo o <b>pepper_fingerprint</b> para detecção de divergência.",
     "F3"),
    ("P1", "Unificar em 12 o mínimo de senha de backup no servidor",
     "Compartilhar uma única constante entre os dois caminhos de backup. Correção de uma linha, que "
     "sustenta diretamente a mitigação de F3.",
     "F4"),
    ("P2", "Remover telefone e e-mail em claro do índice de busca",
     "Migrar <b>patient_search_tokens</b> para tokens derivados irreversíveis por n-grama e adicionar a "
     "coluna <b>user_id</b> à tabela. Exige migration de reindexação e alteração da consulta de busca.",
     "F1"),
    ("P2", "Verificar e neutralizar o projeto Firebase exposto",
     "Confirmar se <b>atendemente-432ab</b> ainda existe; se sim, rotacionar a chave, restringi-la e "
     "revisar as regras de Firestore e Storage. Depende de acesso ao console, não do código.",
     "F5"),
    ("P2", "Alinhar as duas políticas de CSP e travar a divergência com teste",
     "Remover <b>'unsafe-inline'</b> de <b>script-src</b> no tauri.conf.json e estender o teste de "
     "middleware.rs para comparar as duas políticas.",
     "F6"),
    ("P3", "Fechar a enumeração de contas em /auth/recover",
     "Rate limit sobre o e-mail informado, antes de resolvê-lo, e resposta genérica — o mesmo "
     "tratamento que o login já recebeu.",
     "F7"),
    ("P3", "Exigir confirmação do upload no download, ou derivar o Content-Type do conteúdo",
     "Fecha o único caminho em que um registro de arquivo é servido sem ter passado pela validação de "
     "magic bytes.",
     "F8"),
    ("P3", "Ativar a crate validator nos modelos de entrada",
     "A variante de erro já existe e está formatada; falta declarar os limites e chamar "
     "<b>validate()</b> nos handlers.",
     "F9"),
    ("P3", "Empacotar as fontes localmente e fechar a CSP",
     "Elimina a requisição externa a cada abertura e permite remover dois domínios da política.",
     "F10"),
]

COBERTURA = [
    ("Rotas de dados (api/routes.rs)", "34 registros de rota", "Todos revisados"),
    ("Rotas de autenticação (auth/mod.rs)", "9 rotas", "Todas revisadas"),
    ("Comandos Tauri (commands.rs, main.rs)", "6 comandos", "Todos revisados"),
    ("Arquivos Rust do backend", "31 arquivos, ~12.300 linhas", "Todos lidos ou varridos"),
    ("Consultas SQL literais", "Extração automática + revisão manual", "100% classificadas"),
    ("Migrations", "14 de dados + 5 de autenticação", "Todas lidas"),
    ("Arquivos do frontend", "35 arquivos .ts/.tsx em src/", "Varredura por sinks + leitura dirigida"),
    ("Configuração e deploy", "tauri.conf.json, capabilities, vite, CI", "Todos lidos"),
    ("Histórico do git", "Todos os commits alcançáveis", "Varrido com git log -S"),
]
