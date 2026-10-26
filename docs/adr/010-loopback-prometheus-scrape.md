# ADR-010: exporter Prometheus pull local e opt-in

- **Status:** Aceito para a primeira fatia de M5; Remote Write e laboratório pendentes
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 9.1–9.3 e incremento M5.

## Contexto

O plano exige endpoint Prometheus/OpenMetrics local, sem envio externo por
default, sem bloquear a coleta e sem exportar identificadores privados. O
workspace já tem Tokio para runtime e IPC; não tinha servidor HTTP nem cliente de
exportação.

## Decisão

- Implementar a primeira superfície pull em um crate `exporters` separado, usando
  o snapshot imutável já coletado pelo agente. O exporter não inicia coletores.
- Desabilitar por default. Quando ativado em TOML, aceitar apenas endereço
  loopback com porta explícita; endereço não loopback é erro de configuração.
- Servir `GET /metrics` em OpenMetrics 1.0, limitado a 4 scrapes concorrentes,
  cabeçalho de até 4 KiB e timeout de 2 s. Ao atingir capacidade, fechar novas
  conexões sem criar tarefas de rejeição sem limite.
- Limitar a 128 séries por família de dispositivos/volumes/baterias/GPU e 512
  núcleos; publicar `native_monitor_exporter_truncated_series` quando o limite
  omitir alguma série.
- Permitir allowlist de famílias (`cpu`, `memory`, `storage`, `network`, `gpu`,
  `battery`, `health`). Lista vazia preserva todas as famílias; configuração
  desconhecida é rejeitada. As métricas de saúde do agente/exporter permanecem
  disponíveis quando qualquer allowlist está ativa.
- Exportar somente gauges agregados/topologia de dispositivos com aliases
  (`block0`, `interface0`, `gpu0`, `battery0`), nunca nome do dispositivo,
  modelo, serial, nome de usuário ou métricas por processo. Campos ausentes e
  não finitos não produzem amostras.

## Alternativas

- Remote Write como primeira via: adiada porque exige destino, autenticação,
  resiliência, fila e gestão de credenciais; pull local é útil sem credenciais e
  segue a ordem de M5.
- Endpoint em interface externa por default: rejeitado por ampliar a superfície
  de rede e contrariar a exposição local e opt-in do plano.
- Uma tarefa Tokio por conexão sem limite: rejeitada por permitir crescimento de
  tarefas/memória se scrapes locais se acumularem.

## Consequências e revisão

Essa fatia demonstra configuração, renderização e scrape loopback; não
implementa ainda Remote Write, fila/backoff, credential
stores, testes de receptor VictoriaMetrics/Prometheus nem laboratórios Docker.
Revisitar os limites concorrentes após benchmark de scrape representativo; nunca
ampliar bind além de loopback sem decisão e revisão de segurança próprias.
