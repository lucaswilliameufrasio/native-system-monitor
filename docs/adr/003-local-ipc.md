# ADR-003: IPC local versionado

- **Status:** Aceito para o primeiro slice; validação multiplataforma pendente
- **Data:** 2026-10-23
- **Referência:** `docs/PLANO_DESENVOLVIMENTO_MONITOR_SISTEMA.md`, seção 8.1 e
  incremento M0.

## Contexto

Agente, GUI e CLI precisam trocar snapshots tipados sem iniciar novas coletas,
sem expor uma porta de rede e sem deixar clientes lentos acumularem trabalho.

## Decisão

Usar mensagens protobuf geradas por Prost sobre sockets Unix por usuário e named
pipes no Windows. O framing usa prefixo de 4 bytes big-endian e máximo de 1 MiB.
Versões major/minor são carregadas no envelope. A conexão tem timeout de 3 s;
admitem-se no máximo oito clientes ativos, sem fila de espera quando todos os
slots estão ocupados. Em Unix, o socket usa modo `0600` e o peer UID é conferido.
No Windows, a criação do pipe usa descriptor de segurança limitado ao owner;
essa ACL ainda requer teste em runner Windows.

## Alternativas consideradas

- **TCP em loopback:** rejeitado porque não oferece a mesma identidade e
  autorização local das primitivas IPC.
- **JSON para snapshots:** rejeitado para o stream principal; protobuf mantém o
  contrato tipado/versionado e limita o overhead de codificação.
- **Uma thread por cliente sem limite:** rejeitada por permitir crescimento sem
  limite de trabalho em clientes lentos.

## Consequências e revisão

Mensagens e números de campo em `proto/native_monitor/v1/monitor.proto` são
contrato; mudanças aditivas e incompatíveis precisam de testes de versão. Quando
os oito slots estão ocupados, a nova conexão é fechada; a CLI/UI devem exibir
falha de conexão e tentar novamente na próxima consulta. A implementação Windows
e a negociação de compatibilidade precisam de ensaio antes de declarar IPC
validado nessa plataforma. Rever limites com testes de latência, payload e
clientes lentos.
