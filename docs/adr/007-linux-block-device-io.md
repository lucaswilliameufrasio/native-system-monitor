# ADR-007: counters Linux de I/O para dispositivos de bloco

- **Status:** Aceito para a fatia Linux de M2; homologação multiplataforma pendente
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 3.4, 6.1 e incremento M2; ADR-005.

## Contexto

O espaço de um volume montado não representa throughput nem atividade do
dispositivo físico. No Linux, `/proc/diskstats` fornece counters cumulativos de
leituras, escritas, setores e tempo de I/O; sysfs identifica partições e
dispositivos com dependências (por exemplo device-mapper e RAID).

## Decisão

- Implementar parser Linux independente para `/proc/diskstats`, associado ao
  dispositivo por major:minor em `/sys/dev/block`, nunca por substring do nome.
- Excluir partições e dispositivos com `slaves`, evitando dupla contagem das
  camadas inferiores e superiores. Se a topologia sysfs não puder ser lida, não
  incluir o dispositivo como se fosse independente.
- Calcular bytes/s usando setores de 512 bytes, IOPS pelos counters de operações
  concluídas e busy% pelo delta de `io_millis`/tempo monotônico, limitado a
  0–100%. Primeiro sample, resets e intervalos inválidos mantêm taxa ausente.
- Expor métricas explicitamente como I/O de dispositivo de bloco; manter
  volumes montados separados. O coletor informa unsupported fora do Linux até
  existir e ser homologado um adaptador específico.

## Alternativas consideradas

- **Usar nomes de discos ou mounts para deduplicar:** rejeitada por ser frágil
  para NVMe, device-mapper, RAID e partições.
- **Somar todas as linhas de diskstats:** rejeitada porque conta camadas
  empilhadas mais de uma vez.
- **Inferir I/O físico com a API genérica de volumes:** rejeitada por misturar
  filesystem e dispositivo e por não fornecer a semântica necessária.

## Consequências e revisão

Os valores refletem o que o kernel reporta por major:minor e não identificam
latência, saúde, modelo ou tipo físico. A fatia requer fixtures de parser e
topologia, serialização e teste de carga/fonte independente. Rever o filtro em
hardware real com device-mapper/RAID e ao implementar adapters macOS/Windows.
