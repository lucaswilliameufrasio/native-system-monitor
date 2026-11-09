# Changelog

As mudanças relevantes deste projeto serão registradas neste arquivo.

## [Unreleased]

### Adicionado

- Interface desktop e rótulos acessíveis localizados para pt-BR e en-US,
  incluindo detalhes da árvore de processos, resumos de gráficos e estados
  dinâmicos de telemetria.
- Preferências SI/IEC de unidades de bytes e confortável/compacta de densidade
  por usuário; IEC e densidade confortável continuam sendo os padrões.
- Telemetria Linux de bateria, I/O de dispositivo de bloco e volume montado, e
  classificação de interfaces de rede; descoberta/métricas experimentais de GPU
  AMD e Intel.
- Consulta somente de leitura à árvore de processos no agente, IPC, interface
  desktop e CLI.
- Controle de processos no escopo do usuário: pedido gentil, espera de cinco
  segundos e confirmação separada para a ação forçada, com resultados localizados
  e registro local sem command line.
- Benchmark manual de desempenho da UI com renderer Slint software para cenários
  sintéticos de 10.000 processos e gráficos com 1.200 pontos.

### Alterado

- Taxas de I/O de volumes usam baselines de counters por ponto de montagem e
  tempo monotônico; I/O de dispositivo continua sendo uma métrica separada.
- CI audita advisories RustSec e as licenças declaradas das dependências no Linux;
  Clippy, testes e builds exigem o lockfile versionado.
- Licença do projeto definida como MIT, com `LICENSE` e aviso de copyright de
  2026.
- Distribuição macOS planejada com `.app`/`.dmg` sem assinatura/notarização;
  checksums/proveniência continuam previstos para releases.

### Corrigido

- O cabeçalho de Armazenamento mantém espaçamento visível entre a contagem de
  volumes e o seletor de unidades.
- Dependências transitivas atualizadas para remover um advisory RustSec de alta
  severidade e uma versão yanked.

### Notas de validação

- Capacidades de GPU AMD e Intel continuam `unsupported` até homologação em
  hardware real. O benchmark da UI usa renderer de teste software e não mede
  input-to-frame nativo. A opção de densidade extra compacta do plano ainda não
  foi definida nem implementada.
- Este é um resumo de desenvolvimento não lançado, não um registro de release.
  Homologação de instaladores e proveniência continuam pendentes. A allow-list
  de terceiros foi aprovada e registrada em ADR-018; o novo passo de CI ainda
  aguarda validação remota.
