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
slots estão ocupados. Em Unix, o socket fica dentro de um diretório por usuário
com modo `0700`, evitando depender de `fchmod` no descritor do socket; o peer UID
também é conferido. No Windows, a criação do pipe usa descriptor de segurança
limitado ao owner; essa ACL ainda requer teste em runner Windows.

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

### Extensão M1 — consulta de processos (protocolo 1.2)

O envelope da requisição acrescenta `process_filter`, `process_sort_key` e
`sort_descending` como campos protobuf aditivos. O agente aplica a busca por nome
ou PID e a ordenação sobre a lista completa coletada antes de limitar a resposta
a no máximo 1.000 processos. O filtro aceita até 256 caracteres. A resposta
mantém o minor do agente; a GUI só presume filtro/ordenação no agente quando a
resposta anuncia minor 2 ou superior.

Clientes anteriores continuam decodificando as mensagens aditivas. Para uma GUI
1.2 conectada a um agente anterior a 1.2, a compatibilidade recai na filtragem e
ordenação local da resposta limitada; portanto, esse fallback não promete achar
processos que ficaram fora dos 1.000 enviados pelo agente antigo. Instalações
pareadas na mesma versão mantêm a consulta global no agente.

### Extensões M2 — volumes montados e interfaces (protocolo 1.3)

`Snapshot` acrescenta a lista aditiva `volumes`, com ponto de montagem, sistema
de arquivos, capacidade total e espaço disponível opcionais. Clientes antigos
ignoram o campo; clientes novos verificam `protocol_minor >= 3` antes de
interpretar uma lista vazia como ausência de volumes. O campo descreve volumes
montados, não dispositivos físicos nem suas métricas de I/O. A lista aditiva
`network_interfaces` contém nome local da interface e taxas de recebimento/envio
derivadas dos counters desde a amostra anterior; taxas ausentes continuam
ausentes. Os nomes não classificam interface física/virtual e não devem ser
somados como se fossem tráfego físico sem deduplicação.

O benchmark sintético M1 mede filtragem/ordenação de 10.000 processos e
serialização protobuf da resposta limitada. Essa medição não substitui a medição
end-to-end da interação e renderização da GUI.
