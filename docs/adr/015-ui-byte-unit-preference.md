# ADR-015: preferência persistida de unidades de bytes da GUI

- **Status:** Aceita; IEC continua como padrão para preservar a apresentação existente
- **Data:** 2026-11-07
- **Referências:** plano mestre, seções 3.4–3.5 e incremento M7; a preferência
  de densidade da GUI, que compartilha o arquivo local de configurações, está
  especificada em [ADR-016](016-persist-ui-density-preference.md).

## Contexto

O plano exige escolha entre unidades SI e IEC para tamanhos. A GUI exibia
unidades IEC fixas e não tinha armazenamento próprio de preferências; o TOML do
agente contém somente configuração de coleta/exportação e não deve controlar a
apresentação da GUI.

## Decisão

- Oferecer SI (`kB`, `MB`, `GB`, base 1000) e IEC (`KiB`, `MiB`, `GiB`, base
  1024) na página Armazenamento. A escolha afeta bytes, taxas, memória total/swap,
  VRAM e memória por processo; não altera os valores IPC nem as unidades de CPU,
  percentuais, IOPS ou latência.
- Preservar IEC como padrão. A seleção é preferência por usuário e persiste em
  `ui-settings.toml` no diretório de configuração convencional do sistema
  (`XDG_CONFIG_HOME`/`~/.config`, `~/Library/Application Support` ou `%APPDATA%`).
- Arquivo ausente mantém o padrão IEC. Arquivo inválido é ignorado com aviso e
  mantém o padrão; falha de gravação não impede a troca durante a sessão e é
  reportada no log. Nenhum dado de hardware é gravado nesse arquivo.
- A preferência é local à GUI e não é enviada ao agente/IPC.

## Alternativas

- **Configurar unidades no agente:** rejeitada; escolha de apresentação não deve
  alterar snapshots nem configuração de coleta/exportação.
- **Somente preferência de sessão:** rejeitada; não atende à preferência de
  unidade após reiniciar a GUI.
- **Trocar o padrão para SI:** rejeitada nesta fatia; IEC já é o comportamento
  existente e mantê-lo evita mudança silenciosa de interpretação.

## Consequências e revisão

A escolha precisa cobrir todas as quantidades de bytes mostradas pela GUI e deve
ser testada por leitura após gravação. Falha ao ler configurações não pode impedir
a abertura da janela. Reavaliar o escopo se outras preferências da GUI passarem a
compartilhar um esquema persistente.
