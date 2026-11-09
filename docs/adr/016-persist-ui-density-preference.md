# ADR-016: preferência persistida de densidade da GUI

- **Status:** Aceita conforme o requisito de densidade persistida do plano M7
- **Data:** 2026-11-08
- **Referências:** [ADR-015](015-ui-byte-unit-preference.md), plano mestre, seções 3.1 e M7

## Contexto

Antes desta decisão, a GUI oferecia densidades confortável e compacta, mas a
escolha existia somente durante a sessão. O plano M7 pede que a densidade seja
uma preferência por usuário persistida. A GUI já mantém `ui-settings.toml` em
diretório convencional do usuário para a escolha SI/IEC, com IEC como padrão.

## Decisão

- Persistir a densidade no mesmo `ui-settings.toml`, separada da escolha de
  unidades, sem enviar a preferência ao agente ou alterar coleta/IPC.
- Manter confortável como padrão. Arquivos antigos sem o campo `density` seguem
  válidos e recebem esse padrão ao carregar.
- Persistir somente as escolhas já implementadas (`comfortable` e `compact`).
  Não criar a opção extra compacta até que tamanho/spacing e critérios de UX
  sejam definidos; não inferir esses valores.
- Se a escrita falhar, aplicar a escolha na sessão e registrar o erro, sem
  impedir o uso da GUI. Sem diretório de configuração, a preferência é somente
  da sessão e isso deve ser reportado no log.

## Alternativas

- **Manter somente estado de sessão:** rejeitada porque não satisfaz a
  persistência solicitada no plano.
- **Criar outro arquivo de preferências:** rejeitada por duplicar a fronteira
  existente de configuração local da GUI.
- **Adicionar extra compacta com espaçamentos escolhidos nesta fatia:**
  rejeitada por falta de medidas/decisão visual aprovadas.

## Consequências e revisão

A gravação de densidade deve preservar a escolha SI/IEC, e a gravação de
unidades deve preservar a densidade; testes verificam leitura após escrita e
compatibilidade com arquivos antigos. Reavaliar este ADR quando houver uma
definição aprovada para a densidade extra compacta ou quando o esquema de
preferências precisar de migração/transações atômicas.
