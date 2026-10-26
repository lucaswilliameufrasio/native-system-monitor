# ADR-006: taxas de rede por interface a partir de counters

- **Status:** Aceito para a primeira fatia M2; classificação física/virtual pendente
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 3.4, 6.1 e incremento M2; ADR-003.

## Contexto

A GUI e a CLI precisam mostrar atividade por interface sem coletar pacotes ou
somar indiscriminadamente tráfego de interfaces virtuais, túneis e loopback.
`sysinfo` expõe counters cumulativos portáveis e os seus deltas podem ser
normalizados pela duração monotônica entre amostras.

## Decisão

- Enumerar interfaces e ler counters cumulativos com `sysinfo` no agente comum.
- Derivar bytes/s pela diferença entre duas amostras de uma mesma interface e
  pelo tempo monotônico decorrido. A primeira amostra, intervalo inválido ou
  counter que diminui (reset) produz taxa ausente até uma nova baseline.
- Expor o nome da interface apenas localmente; não inferir que seja física ou
  somar interfaces relacionadas. A UI/CLI apresentam as taxas por interface.
- Preservar as taxas opcionais no domínio e no protocolo protobuf 1.3.

## Alternativas consideradas

- **Captura de pacotes ou atribuição por conexão:** rejeitada por não ser
  necessária ao caso M2 e aumentar custo, privilégio e sensibilidade de dados.
- **Interpretar o delta de uma chamada como bytes/s sem medir o intervalo:**
  rejeitada porque a cadência de coleta pode variar e gerar unidades incorretas.
- **Classificar interfaces apenas pelo nome:** rejeitada porque nomes não são
  evidência confiável de interface física, virtual, VPN ou loopback.

## Consequências e revisão

Counters reiniciados ou interfaces que acabam de surgir ficam sem taxa até haver
duas amostras válidas. Os nomes continuam locais e não são exportados. Revisar a
fonte e a semântica em testes com carga e hardware de cada OS; implementar
classificação/deduplicação somente com propriedades nativas verificáveis.
