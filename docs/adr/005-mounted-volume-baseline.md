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
