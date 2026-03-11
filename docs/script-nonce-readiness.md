# Readiness de Script Nonce - AtendeMente

## Resumo executivo

O AtendeMente esta pronto para iniciar um rollout controlado de `script nonce`, mas **nao** de forma global e imediata. No estado atual da branch de teste:

- nao ha uso explicito de `next/script`, `<script>` manual, `dangerouslySetInnerHTML` ou `nonce` no codigo do app
- a CSP enforced ainda usa `unsafe-inline` para `script-src`
- ja existe uma `Content-Security-Policy-Report-Only` mais estrita e um endpoint de coleta em `/api/security/csp-report`
- o app mistura rotas dinamicas e estaticas, e no Next 15 o nonce continua sendo um valor **por request**, o que empurra as rotas afetadas para renderizacao dinamica

Conclusao pratica:

- **Fase 2 recomendada**: aplicar `script nonce` apenas nas rotas autenticadas do app
- **Fase 3 recomendada**: avaliar paginas publicas sensiveis (`/login`, `/forgot-password`, `/reset-password`, `/accept-invite`) depois de medir latencia, erro e relatorios CSP
- **Fase 4 recomendada**: manter paginas legais estaticas fora do rollout, salvo exigencia comercial especifica

## Leitura sobre Next 15 e Next 16

### Next atual

No Next 15, `script nonce` continua sendo um valor por request. Isso significa que qualquer rota HTML que adote nonce de forma enforced precisa participar de um fluxo dinamico, mesmo que hoje seja renderizada como estatica.

### Next 16

O Next 16 melhora a historia de cache e partial prerendering com recursos como Cache Components/PPR, o que pode reduzir o custo percebido de rotas dinamicas. Isso **nao** elimina a necessidade fundamental do nonce por request. Em outras palavras:

- Next 16 pode ajudar a amortecer impacto de performance
- Next 16 **nao** remove o requisito arquitetural do nonce
- o upgrade nao e pre-requisito para iniciar o rollout nas rotas autenticadas

Decisao desta fase:

- manter o projeto em **Next 15**
- tratar o upgrade como opcional e posterior

## Inventario atual por rota

Baseado no `next build` atual da branch de teste.

| Grupo | Rotas | Estado atual | Classificacao de impacto | Leitura |
| --- | --- | --- | --- | --- |
| App autenticado | `/dashboard`, `/agenda`, `/patients`, `/patients/[patientId]`, `/appointments/[appointmentId]`, `/financeiro` | dinamicas | `ja dinamica` | melhor candidata para Fase 2 |
| Shell/redirect publico | `/`, `/login` | dinamicas | `ja dinamica` | tecnicamente aptas, mas adiar para Fase 3 por serem paginas publicas |
| Auth estatico | `/accept-invite`, `/forgot-password`, `/reset-password` | estaticas | `estatica com baixo impacto` | baixo trafego e maior beneficio de seguranca, candidatas para Fase 3 |
| Legais | `/privacidade`, `/termos`, `/lgpd` | estaticas | `estatica com alto impacto` | manter estaticas, baixo ganho de seguranca e custo desnecessario |
| Not found | `/_not-found` | estatica | `estatica com alto impacto` | fora do rollout |
| APIs | `/api/*` | dinamicas | nao aplicavel para `script nonce` | monitorar separadamente, sem escopo de nonce |

## Mapeamento de dependencias de nonce

### O que foi confirmado no codigo

- nao ha `next/script`
- nao ha tags `<script>` author-defined
- nao ha `dangerouslySetInnerHTML`
- nao ha scripts de terceiros author-defined hoje

### O que provavelmente vai aparecer nos bloqueios

Como o app nao injeta scripts inline manualmente, os candidatos mais provaveis para relatorio CSP sao:

- bootstrap inline do proprio Next.js
- payloads inline do React/Flight gerados pelo framework
- ruido de extensoes do navegador

### Como separar sinal de ruido

Na analise dos reports de `/api/security/csp-report`, tratar como:

- **bloqueio real do app/framework**:
  - `violated-directive` relacionado a `script-src`
  - `blocked-uri` igual a `inline`
  - ocorrencia reproduzivel em navegador limpo
- **ruido externo**:
  - origem de extensao
  - user-agent com plugins conhecidos
  - eventos nao reproduziveis sem extensoes

## Impacto esperado de performance e arquitetura

### Areas autenticadas

- impacto esperado: **baixo a moderado**
- motivo: essas paginas ja sao dinamicas e dependem de sessao, banco e dados por usuario
- custo adicional esperado: geracao e propagacao do nonce por request e pequeno ajuste no caminho de renderizacao/headers

### Paginas publicas de auth

- impacto esperado: **moderado**
- motivo: hoje parte delas e estatica e teria de entrar em fluxo dinamico
- compensacao: sao paginas de baixo trafego e alto valor de seguranca

### Paginas legais

- impacto esperado: **alto**
- motivo: perderiam simplicidade e beneficio de estatico sem ganho relevante
- decisao recomendada: manter fora do rollout inicial

## Rollout recomendado para a fase de implementacao

### Fase 2 - escopo inicial aprovado

Aplicar `script nonce` apenas em:

- `/dashboard`
- `/agenda`
- `/patients`
- `/patients/[patientId]`
- `/appointments/[appointmentId]`
- `/financeiro`

Decisao:

- manter `/login` fora da primeira onda mesmo sendo dinamica
- manter todas as APIs fora do escopo de nonce

### Estado atual da branch de teste

Esta fase 2 ja esta implementada em modo `report-only` na branch de teste:

- o `middleware` gera `x-csp-nonce` apenas para as rotas autenticadas dentro do escopo inicial
- as rotas autenticadas recebem `Content-Security-Policy-Report-Only` com `script-src 'nonce-...'`
- a CSP enforced global permanece inalterada
- o layout autenticado le o nonce via `headers()` para estabilizar o caminho do App Router
- os reports seguem para `/api/security/csp-report` com logs estruturados enriquecidos

### Fase 3 - extensao opcional

Apos medir producao e revisar CSP reports:

- `/login`
- `/forgot-password`
- `/reset-password`
- `/accept-invite`

### Fase 4 - fora do rollout por padrao

- `/privacidade`
- `/termos`
- `/lgpd`
- `/_not-found`

## Plano fechado para implementacao futura

### Onde o nonce sera gerado

- no `middleware`, com valor aleatorio por request
- o valor deve ser colocado em header interno da request, por exemplo `x-csp-nonce`

### Como o nonce sera propagado

- o `middleware` deve propagar o nonce para as rotas incluídas no rollout
- layouts e paginas afetadas devem ler o nonce via `headers()`
- a CSP enforced dessas rotas deve passar a usar `script-src 'self' 'nonce-<valor>'`
- a politica `report-only` continua ativa durante o rollout inicial

### Como ler os reports nesta fase

- focar em eventos `security.csp_report`
- priorizar registros com:
  - `violatedDirective` ou `effectiveDirective` relacionados a `script-src`
  - `blockedUri = inline`
  - `documentUri` pertencente a `/dashboard`, `/agenda`, `/patients`, `/appointments` ou `/financeiro`
- tratar como ruido:
  - relatorios ligados a extensoes
  - user-agents com overlays/autofill conhecidos
  - origens nao reproduziveis em navegador limpo

### O que fica fora desta rodada

- `style nonce`
- mudanca global para todas as paginas
- upgrade para Next 16

### Sinais de sucesso da fase 2

- nenhuma quebra visual ou de navegacao nas rotas autenticadas
- ausencia de aumento relevante de erro 5xx nessas rotas
- ausencia de regressao perceptivel de latencia no dashboard e agenda
- CSP reports concentrados no que restar do framework, nao em codigo autoral do app
- logs estruturados com `requestId` suficientes para investigar falhas de rollout

## Bloqueios e riscos conhecidos

- o framework do Next provavelmente continuara exigindo inline bootstrap em algum nivel; o rollout deve ser medido e nao presumido
- o `script nonce` precisa ser restrito por grupo de rota para evitar transformar paginas estaticas sem necessidade
- a coleta de `CSP report` ainda precisa de observacao real em ambiente publicado para separar ruido de extensoes

## Definicao de pronto para a proxima fase

A implementacao de `script nonce` pode comecar quando estes criterios estiverem atendidos:

1. logs de `CSP report` em ambiente publicado tiverem sido revisados por alguns dias
2. a decisao de escopo inicial permanecer nas rotas autenticadas
3. o time aceitar que paginas do app autenticado continuem dinamicas
4. a validacao de sucesso incluir `lint`, `test`, `build` e smoke das rotas autenticadas
