# ADR-009: telemetria Linux de bateria por sysfs

- **Status:** Aceito para a primeira fatia Linux de M4; SMART e outros OS pendentes
- **Data:** 2026-10-26
- **Referência:** plano mestre, seção 3.6 e incremento M4.

## Contexto

O agente precisa exibir estado e contadores de bateria sem ativar repetidamente
discos ou exigir privilégios. Linux publica atributos de fontes de energia em
`/sys/class/power_supply`; os atributos variam entre drivers e dispositivos.

## Decisão

- Enumerar somente entradas cujo atributo `type` seja `Battery` e ler arquivos
  sysfs passivamente. Não executar `smartctl`, abrir dispositivos de bloco nem
  solicitar elevação nesta fatia.
- Tratar cada campo independentemente como opcional. `energy_*` e `power_now`
  são convertidos de µWh/µW para Wh/W; quando power não existir, derivar W de
  `current_now` (µA) × `voltage_now` (µV). Temperatura `temp` segue décimos de °C.
- Expor capacidade apenas no intervalo 0–100; ciclos e estado são fornecidos
  pelo driver quando presentes. Ausência do diretório/baterias resulta em
  `unsupported`, não em valores zero.
- Manter os valores no snapshot IPC local e apresentar um resumo na GUI/CLI.

## Alternativas

- Consultar `upower` ou executar `smartctl`: rejeitadas para a leitura padrão por
  dependerem de processo/serviço ou acesso a disco, e por não serem necessárias
  para as métricas básicas de bateria.
- Inferir capacidade/saúde quando o driver não informa o dado: rejeitada porque
  cria precisão falsa; não se infere percentagem de saúde.

## Consequências e revisão

Leituras são de baixo privilégio e não fazem polling de SMART. A primeira
implementação cobre o contrato comum do sysfs e valida fixtures; permanece
necessário testar baterias de outras famílias, adapters macOS/Windows, SMART
opt-in seguro, permissões e o comportamento físico de energia/hotplug.
