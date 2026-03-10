# Checklist de lancamento - AtendeMente

## Gate obrigatorio de release

Sempre execute:

```bash
npm run verify:release
```

Nenhuma publicacao deve seguir com `lint`, `test`, `build` ou `test:e2e` falhando.

## Configuracao Vercel + Supabase

- Confirmar `NEXT_PUBLIC_APP_URL`, `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY`, `SUPABASE_SERVICE_ROLE_KEY`, `DATABASE_URL`, `APP_ENCRYPTION_KEY` e `PRIVATE_STORAGE_BUCKET`.
- Aplicar todas as migrations de `supabase/migrations`.
- Revisar `Site URL` e `Redirect URLs` do Supabase.
- Revisar templates de convite e redefinicao de senha.

## Operacao minima

- Validar convite real por email.
- Validar reset de senha real por email.
- Validar upload e download de anexo.
- Validar exportacao ZIP de paciente.
- Revisar logs recentes na Vercel e no Supabase.
- Revisar logs de auditoria para login, exportacao e download.

## Backup e restauracao

- Definir rotina de backup do banco no Supabase.
- Registrar onde o backup fica e quem pode acessa-lo.
- Testar restauracao em ambiente separado.
- Testar exportacao de paciente como mecanismo complementar de portabilidade.

## Seguranca

- Confirmar bucket `private-record-files` como privado.
- Confirmar que `request_limits` existe no banco publicado.
- Confirmar que links legais publicos (`/privacidade`, `/termos`, `/lgpd`) estao publicados.
- Confirmar que registros criptografados continuam legiveis no produto.

## Suporte e rotina comercial

- Definir procedimento de convite, reenvio e expiracao.
- Definir procedimento de recuperacao de acesso.
- Registrar responsavel por revisar falhas nas primeiras semanas.
- Reforcar que o formato oficial de importacao em lote e `CSV`.
