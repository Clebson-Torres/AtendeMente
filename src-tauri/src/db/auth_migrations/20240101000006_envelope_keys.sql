-- Criptografia envelopada: a chave que abre os dados passa a depender da senha.
--
-- Hoje a chave AES sai de HKDF(salt = pepper_do_cofre, ikm = user_id). A senha
-- nao participa, entao quem tem a conta do sistema operacional deriva a chave e
-- le todo o prontuario sem saber a senha — demonstrado na pratica.
--
-- No modelo envelopado existe uma DEK aleatoria por usuario, guardada apenas
-- embrulhada: uma copia sob a senha da conta e outra sob o codigo de
-- recuperacao. Sem um dos dois segredos, nao ha caminho para a DEK.
--
-- POR QUE ESTAS TABELAS FICAM NO BANCO DE AUTENTICACAO, e nao no do usuario:
--
-- `reset_password` troca `password_hash` e `recovery_secret_hash` num unico
-- UPDATE. O reembrulho da DEK tem de estar na MESMA transacao — em bancos
-- diferentes seriam dois commits, e uma queda entre eles deixaria a DEK sem
-- nenhum wrap valido, o que e perda permanente e silenciosa.
--
-- Efeito colateral desejavel: o `VACUUM INTO` do backup copia so o banco do
-- usuario, entao o .db dentro do bundle nao carrega os wraps. E `auth.db` nao e
-- restaurado, logo o `DELETE FROM` que o import faz em cada tabela nao tem como
-- apagar o envelope — o que aconteceria se ele morasse numa tabela restaurada.

CREATE TABLE IF NOT EXISTS user_deks (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
    -- SHA-256(dek) truncado: identifica qual DEK sem revelar nenhum material.
    -- E o que permite verificar antes de descartar a chave antiga.
    dek_check  TEXT NOT NULL,
    -- 'current' | 'retiring'. Durante a rotacao as duas coexistem, e a antiga
    -- so e removida depois de todo dado provar que abre sob a nova.
    role       TEXT NOT NULL,
    -- 'random' (DEK propria) | 'legacy_pepper_v1' (a chave derivada do pepper,
    -- registrada como ponto de partida da migracao).
    source     TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_deks_current
    ON user_deks(user_id) WHERE role = 'current';
CREATE INDEX IF NOT EXISTS idx_user_deks_user ON user_deks(user_id);

CREATE TABLE IF NOT EXISTS dek_wraps (
    dek_id     TEXT NOT NULL REFERENCES user_deks(id) ON DELETE CASCADE,
    -- 'password' | 'recovery' | 'recovery_prev'.
    -- `recovery_prev` sobrevive ate o usuario confirmar que anotou o codigo
    -- novo: sem isso, um reset em que ele perde o codigo emitido e esquece a
    -- senha significa perda total, sem caminho de suporte.
    slot       TEXT NOT NULL,
    kdf        TEXT NOT NULL,
    -- Parametros gravados POR LINHA, nunca lidos de constante: eles sobem com o
    -- tempo, e um wrap antigo tem de continuar abrindo com os parametros com que
    -- foi criado.
    m_cost     INTEGER NOT NULL,
    t_cost     INTEGER NOT NULL,
    p_cost     INTEGER NOT NULL,
    salt       TEXT NOT NULL,
    nonce      TEXT NOT NULL,
    wrapped    TEXT NOT NULL,
    -- String exata usada como dado autenticado adicional. Amarra o wrap ao
    -- usuario e ao slot: um wrap de senha nao pode ser movido para o slot de
    -- recuperacao nem para a linha de outro usuario.
    aad_label  TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (dek_id, slot)
);
