# ADR-002: agente separado da GUI

- **Status:** Aceito
- **Data:** 2026-10-23
- **Referência:** `docs/PLANO_DESENVOLVIMENTO_MONITOR_SISTEMA.md`, decisões 1.1,
  seção 4.1 e incremento M0.

## Contexto

A GUI, CLI e exportadores precisam compartilhar uma coleta por cadência. Fechar
a janela não deve interromper a coleta nem o histórico em memória.

## Decisão

Executar coleta e estado de curto prazo em um processo agente independente. GUI e
CLI são clientes IPC; o agente coleta uma vez por intervalo e serve snapshots
cacheados. A GUI não coleta diretamente nem controla o ciclo de vida do agente.

## Alternativas consideradas

- **Coleta dentro da GUI:** rejeitada porque fecha junto com a janela e poderia
  duplicar sondagens quando houver mais clientes.
- **Helper privilegiado como coletor principal:** rejeitada; privilégios não são
  necessários para a coleta inicial e o helper deve permanecer opcional e
  restrito.

## Consequências e revisão

O agente ainda é iniciado manualmente neste incremento. Instalação, início no
login e modos de serviço ficam para as etapas de empacotamento/operação do plano.
IPC precisa limitar frames, clientes e tempo por solicitação, sem bloquear a
coleta. Reavaliar apenas se requisitos de continuidade, isolamento ou consumo
medidos demonstrarem que o boundary precisa mudar.
