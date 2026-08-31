# Auditoria de segurança

Relatório da auditoria de segurança do AtendeMente e o gerador que o produz.

## Arquivos

| Arquivo | O que é |
|---|---|
| `relatorio-auditoria-seguranca.pdf` | O relatório. É o entregável. |
| `issues-github.md` | As mesmas issues que estão no fim do PDF, em Markdown puro — copiar daqui evita as quebras de linha impostas pela largura da coluna do PDF. |
| `achados.py` | Os dados: achados, pontos fortes e fracos, recomendações, metodologia. |
| `gerar_relatorio.py` | O layout. Não contém conteúdo de auditoria. |
| `.venv/` | Ambiente isolado com `reportlab`, `matplotlib` e `pymupdf`. Não versionado. |

## Situação dos achados

Todos os onze achados foram tratados. O relatório os mantém como registro do que
existia na v1.0.19 (commit `6ed2d6a`), com uma faixa **Situação atual** em cada um
dizendo como foi resolvido.

| Achado | Severidade | Situação |
|---|---|---|
| F1 · PII em claro no índice de busca | Alta | Migration 15: HMAC de n-gramas + coluna `user_id` |
| F2 · Senha de backup global | Alta | Entrada do cofre por usuário, com migração da legada |
| F3 · Pepper mestre no backup | Alta | Backup v3 com envelope de chave; v1 e v2 ainda restauram |
| F4 · Mínimo de senha divergente | Média | Constante única de 12 caracteres |
| F5 · Chave Firebase no histórico | Média | Projeto `atendemente-432ab` excluído pelo autor |
| F6 · CSP da webview permissiva | Média | `unsafe-inline` removido, com teste comparando as duas |
| F7 · Enumeração em /auth/recover | Baixa | Rate limit antes do lookup, resposta genérica |
| F8 · Content-Type do download | Baixa | Tipo derivado dos magic bytes |
| F9 · Sem validação no servidor | Baixa | `#[validate]` nos sete structs de entrada |
| F10 · Google Fonts | Baixa | Fontes no bundle, domínios fora da CSP |
| F11 · Pepper degenerado aceito | Informativa | Recusado no boot; `hex_decode` endurecido |

A suíte Rust foi de 159 para 181 testes. Um teste flaky pré-existente
(`test_decrypt_tampered_payload_fails`, que falhava ~1 vez a cada 64 execuções) foi
corrigido no caminho.

## Regerar o relatório

```bash
cd docs/security-audit
./.venv/bin/python gerar_relatorio.py
```

Isso reescreve o PDF e o `issues-github.md`.

Para reemitir depois de corrigir um achado, edite `achados.py` — remova o achado, ou
mude sua severidade — e rode o gerador de novo. O gerador recalcula sozinho os totais,
os dois gráficos, a tabela consolidada e as issues; nada disso está fixado no código de
layout.

## Recriar o ambiente

O `.venv/` não é versionado. Numa máquina nova:

```bash
cd docs/security-audit
python3 -m venv .venv
./.venv/bin/pip install reportlab matplotlib pymupdf
```

`pymupdf` é usado só para conferir o resultado (contagem de páginas e rasterização das
páginas para inspeção visual); o relatório é gerado sem ele.

## Conferir o resultado

```bash
./.venv/bin/python -c "
import pymupdf
d = pymupdf.open('relatorio-auditoria-seguranca.pdf')
print(d.page_count, 'páginas')
for i in range(d.page_count):
    d[i].get_pixmap(dpi=90).save(f'/tmp/p{i+1:02d}.png')
"
```

## Estrutura de um achado

Cada entrada de `ACHADOS` alimenta três lugares do PDF ao mesmo tempo — a tabela
consolidada, a análise achado a achado e a issue no fim — então basta editar num lugar:

```python
dict(
    id="F1",
    titulo="...",                    # frase única, aparece nos três lugares
    categoria="1. Banco sem tranca (isolamento por usuário)",
    severidade="alta",               # critica | alta | media | baixa | informativa
    locais=[("caminho/arquivo.rs", "236-252")],
    trecho="""...""",                # código real, com o caminho:linha em comentário
    porque="...",                    # por que é explorável
    impacto="...",
    condicoes="...",                 # o que a exploração exige
    correcao="...",
    aceite=["..."],                  # checklist da issue; lista vazia = não vira issue
    issue_labels="security, alta",
    issue_titulo="[Segurança] ...",
)
```

Os campos de texto aceitam as tags de ênfase do ReportLab (`<b>`, `<i>`); ao montar o
Markdown da issue elas viram crase.

Uma severidade nova precisa entrar em `PALETA` e em `ROTULO_SEV`, e uma categoria nova
em `rotulo_cat` dentro de `gerar_relatorio.py` — o gráfico de barras usa esse mapa para
encurtar os rótulos dos eixos.
