# Fixtures de compatibilidade — backups da v1.0.19

Estes arquivos são a rede de proteção de compatibilidade da reforma de criptografia.
Foram gerados com a v1.0.19 **intocada** (commit `b65fd44`), pela API real do app, e
existem para responder a uma única pergunta em cada etapa da reforma: *um usuário que
tem dados e backups da v1.0.19 continua conseguindo abrir tudo?*

**Todos os dados são fictícios.** Nenhum paciente real, nenhum dado real de saúde.

## Arquivos

| Arquivo | SHA-256 (32 primeiros) | Tamanho |
|---|---|---|
| `backup-v1.0.19-agendado.atendemente` | `c8962e0e35cde05afe56af6df0b3e900` | 32.684 B |
| `backup-v1.0.19-interativo.atendemente` | `775f9ac29bdb0299baec5832bf403033` | 33.217 B |
| `backup-v1.0.19.expected.json` | `ab8e1ad4d362536eb209847e80be1ec8` | 17.765 B |

**Nunca regenerar.** Se algum destes hashes mudar, o fixture deixou de representar a
v1.0.19 e o teste de compatibilidade perdeu o sentido. Eles estão marcados como binários
no `.gitattributes` justamente para o `core.autocrlf` do Git para Windows não reescrever
bytes no checkout — o mesmo problema que já travou o app pelos checksums das migrations.

## Segredos (são de teste, podem estar em texto claro aqui)

```
senha da conta      SenhaDoFixture#2026
e-mail da conta     fixture@exemplo.invalid
senha do backup     SenhaDoBackupFixture#2026
pepper de origem    base64  Zml4dHVyZS1wZXBwZXItZGUtdGVzdGUtMzJieXRlcyE=
                    (= os 32 bytes ASCII "fixture-pepper-de-teste-32bytes!")
```

Nos testes, use um pepper **diferente** deste para reproduzir o cenário de máquina nova:

```
pepper diferente    base64  cGVwcGVyLURJRkVSRU5URS1tYXF1aW5hLW5vdmEhISE=
                    (= "pepper-DIFERENTE-maquina-nova!!!")
```

Atenção: `MASTER_PEPPER` com tamanho diferente de 32 bytes é **ignorado silenciosamente**
e o app cai para o pepper do cofre do sistema. Um teste que se pretende isolado passa a
ler estado de produção. Confira sempre que o log diz *"Usando MASTER_PEPPER do ambiente"*
e não *"MASTER_PEPPER definida mas invalida"*.

## Por que dois backups, e não um

A v1.0.19 produz dois bundles genuinamente diferentes, porque o login roda
`migrate_plaintext_pii` e carrega a chave de dados na memória do processo:

- **`-agendado`** — feito **sem** sessão de criptografia (é o que o agendador de hora em
  hora faz, e o que acontece quando ninguém logou desde o boot). O paciente #12 continua
  no **formato v1**, com a PII nas colunas em texto claro, e os anexos entram no ZIP
  **ainda cifrados**, porque `collect_files` não consegue a chave.
- **`-interativo`** — feito **depois** de um login. O paciente #12 já foi migrado para o
  blob cifrado, ganhou 4 search tokens, e os anexos entram no ZIP **decifrados**.

Os dois precisam restaurar. Não existe fixture "sem senha": tanto o handler HTTP
(`routes.rs`, "Informe uma senha para proteger o backup") quanto o agendador exigem senha,
então um bundle não cifrado é inalcançável na v1.0.19. Para cobrir esse formato legado,
chame `create_backup_with_password(..., None)` direto num teste Rust.

## Conteúdo: 12 pacientes, cada um exercitando um caso frágil

| # | O que exercita |
|---|---|
| 1 | acentos no nome (`José da Conceição`) — normalização, ordenação e busca |
| 2 | `ñ` e `Â` (`Ângela Nuñez`) — ambos dobrados por `normalize_patient_name` |
| 3 | `ä`, `ü`, `ý` (`Jürgen Bäckström Lýdia`) — diacríticos que `normalize_patient_name` **não** dobra |
| 4 | homônimo do #1 com telefone diferente — `identity_key` distinto, não deve colidir |
| 5 | **sem telefone** — `identity_key` termina em `::` |
| 6 | `chart_number` NULL — fora do índice único parcial |
| 7 | `chart_number = "P001"` — baixa entropia, é o caso que torna o blind index obrigatório |
| 8 | e-mail **sem `@`** — `split('@').next()` gera duas linhas idênticas de token |
| 9 | histórico clínico de ~7,7 KB — blob grande |
| 10 | emoji fora do BMP nas anotações — 6 codepoints acima de U+FFFF |
| 11 | `status = 'inactive'` |
| 12 | **formato v1**: PII nas colunas em claro, `pii_encrypted` NULL, sem search tokens. Escrito direto no SQLite, porque é o único jeito de ter esse estado — é o registro que testa `migrate_plaintext_pii` |

Mais: 6 consultas (uma série recorrente semanal de 4 ocorrências e uma cancelada com
`cancel_reason`), 2 pagamentos (um pago via pix com `notes`, um pendente), 2 prontuários
de sessão cifrados, e **3 anexos** — um PDF e um PNG cifrados pela API, e um terceiro PDF
gravado **em texto claro** no disco, que exercita o passthrough de `decrypt_file`
(`crypto.rs:181-183`) e é o estado real de todo anexo restaurado de qualquer backup.

## Como usar

`backup-v1.0.19.expected.json` é o *golden*: a PII decifrada que a API deve devolver,
campo por campo, para os 12 pacientes. Foi conferido contra o que foi enviado na criação —
**zero divergências**, acentos e emoji preservados.

O invariante do teste é: restaurar qualquer um dos dois bundles numa base limpa, **com um
pepper diferente do de origem**, e obter exatamente o golden. Vale para os dois sabores: o
formato de armazenamento difere, o que a usuária vê não.

## Estado conhecido: hoje este teste FALHA

Com a v1.0.19, restaurar com pepper diferente devolve **HTTP 500** e o log mostra:

```
Erro ao ler PII: error returned from database: (code: 1) no such column: key_version
```

É `reencrypt_all_pii` (`crypto.rs:207-210`) consultando uma coluna que `patients` não tem.
E o erro propaga com `?` em `backup.rs:285`, **depois** de `import_database` (`:276`) e
`restore_storage` (`:277`) já terem rodado. Medido neste fixture:

- o banco **é substituído** — 12 pacientes, 6 consultas e os 3 anexos vêm do backup;
- **11 dos 12 pacientes ficam ilegíveis**, porque a re-cifra que consertaria isso é
  exatamente a linha que aborta;
- **nada é registrado em auditoria**, porque o `BackupRestored` fica depois do abort;
- a usuária vê "erro interno" e conclui que nada aconteceu — mas o estado local anterior
  já foi sobrescrito.

O cenário não é exótico: é "reinstalei o Windows e copiei minha pasta de dados". Os bancos
são arquivos e sobrevivem; o Credential Manager não, então o pepper é regenerado com o
mesmo `user_id`.

Corrigir isso é a etapa **R0** do plano. Quando ela estiver pronta, o teste de
compatibilidade deve passar nos dois bundles.
