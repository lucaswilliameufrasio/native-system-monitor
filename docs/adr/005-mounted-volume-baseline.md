# ADR-005: baseline de capacidade para volumes montados

- **Status:** Aceito para a primeira fatia M2; métricas de I/O pendentes
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 3.4, 6.1, 7.1 e incremento M2; ADR-003.

## Contexto

O primeiro incremento de armazenamento precisa mostrar espaço de volumes em uso
sem confundir ocupação de filesystem com saúde ou atividade de um dispositivo
físico. A coleta comum com `sysinfo` está disponível nos três alvos do workspace,
mas não oferece por si só toda a topologia física/volume nem métricas nativas
uniformes.

## Decisão

- Usar `sysinfo` como baseline multiplataforma para enumerar volumes montados,
  sistema de arquivos, capacidade total, espaço disponível e deltas de I/O por
  filesystem normalizados pelo tempo monotônico entre refreshes.
- Tratar o ponto de montagem como identidade local da entrada montada, não como
  ID estável de disco físico. Não inferir relação volume → dispositivo nem
  atribuir I/O físico, latência, temperatura ou saúde a partir desses dados.
- Preservar capacidade desconhecida como ausente no domínio, protobuf, GUI e CLI;
  não converter indisponibilidade em zero.
- Para continuidade de I/O, guardar counters cumulativos e instante monotônico
  por entrada montada (mount, filesystem e nome do dispositivo, sem conversão
  lossy na chave). Primeira observação, reaparecimento após ausência observada,
  troca de filesystem/dispositivo, regressão de counter ou intervalo inválido
  mantêm a taxa correspondente ausente; a amostra atual estabelece a próxima
  baseline. Descartar entradas não enumeradas no ciclo atual.
- Acrescentar o campo protobuf no minor 1.3; clientes antigos ignoram a lista e
  clientes novos distinguem agente anterior de lista vazia.

## Alternativas consideradas

- **Declarar cada mount como disco físico:** rejeitada por misturar filesystem,
  partições e dispositivo e poder contar camadas empilhadas mais de uma vez.
- **Aguardar simultaneamente adaptadores nativos de todos os OS:** rejeitada
  para a primeira jornada de capacidade, pois atrasaria um slice útil e testável.
- **Completar métricas de I/O com `sysinfo` sem validação da semântica:**
  rejeitada; throughput/IOPS/busy-time serão adicionados apenas quando a fonte e
  o denominador estiverem documentados e cobertos por testes.

## Consequências e revisão

A tela Armazenamento e `monitorctl devices --type disk` mostram somente volumes
montados, capacidade e taxas de I/O do filesystem. Hotplug é reenumerado no
ciclo de coleta, sujeito à validação de cada OS. Taxas de volume não substituem
I/O físico; M2 ainda depende de adaptadores nativos e ensaios independentes por
OS. Rever a escolha do baseline se testes reais mostrarem omissões, bloqueios ou
custo excessivo de sondagem.

## Refinamento de continuidade (2026-10-29)

A implementação usava deltas de `DiskUsage` e um único instante global, o que
não distinguia o primeiro refresh de um volume surgido após a inicialização.
O tracker por entrada usa `total_read_bytes`/`total_written_bytes` para detectar
também regressões dos counters antes de calcular bytes/s. Trata-se de correção
do contrato de ausência, sem mudança de schema ou nova identidade física.
Fonte: [sysinfo 0.39.6 — DiskUsage](https://docs.rs/sysinfo/0.39.6/sysinfo/struct.DiskUsage.html).

Fixtures cobrem continuidade, adição, remoção, reaparecimento, troca observável,
resets e intervalos inválidos. Não detectamos uma substituição inteiramente entre
dois ciclos que reutilize mount, filesystem e nome com counters não regressivos;
isso exige identidade nativa futura. A chave não é exportada nem logada. Hotplug
físico e semântica nativa macOS/Windows continuam sem homologação nesta fatia.
