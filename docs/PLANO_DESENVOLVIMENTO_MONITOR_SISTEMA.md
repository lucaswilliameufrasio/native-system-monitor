# Plano mestre de desenvolvimento — Monitor de Sistema Nativo

**Versão do plano:** 1.0 · **Data-base:** 21/10/2026 · **Status:** especificação consolidada para execução incremental  
**Nome de trabalho do repositório:** `native-system-monitor` (provisório; não é decisão de branding)  
**Objetivo:** aplicativo open source, nativo, rápido e confiável para observar e administrar recursos do computador, inspirado na clareza operacional do Gerenciador de Tarefas e do Monitor de Recursos do Windows 7, sem reproduzir seu visual literalmente.

> **Princípio central:** o monitor não pode virar um dos maiores consumidores de recursos do computador monitorado.

---

## 1. Decisões aprovadas e fronteiras

### 1.1 Decisões fechadas

| Tema | Decisão | Consequência arquitetural |
|---|---|---|
| Linguagem | Rust | Workspace Cargo, isolamento de unsafe/FFI e tipos explícitos para métricas. |
| GUI | Slint, com checkpoint técnico | Aplicativo desktop tradicional, sem WebView. Reavaliar só se critérios de tabelas/gráficos/a11y falharem. |
| Arquitetura | Monólito modular, package-by-feature | Bibliotecas compartilhadas, múltiplos executáveis locais; não usar microsserviços. |
| Coleta em segundo plano | **Sim, agente independente** | A GUI é cliente; fechar a janela não interrompe coleta nem exportação. |
| Acesso privilegiado | **Sim, opcional e explicitamente autorizado** | Helper mínimo separado do processo comum; nenhuma GUI root/admin. |
| Licenciamento/distribuição | **Open source** | Preferência inicial por GPL-3.0-only (alinhada à opção GPLv3 da Slint), sujeita à auditoria de compatibilidade das dependências. |
| macOS | **Somente Apple Silicon (arm64)** | Não suportar, compilar, empacotar ou testar macOS Intel. |
| Linux | x86_64 e aarch64 | Wayland e X11 para a GUI, quando presentes; agente independente do display server. |
| Windows V1 | **Windows 11 x64** | Instalador, serviço/agente e coleta nativa Windows. |
| Windows futuro | Windows 10 e Windows ARM64 | Não bloquear a arquitetura; sem promessa de suporte V1. |
| Mac App Store | **Fora do projeto** | Distribuição direta `.app`/DMG; assinatura Developer ID e notarização oficiais. |
| Observabilidade externa | Prometheus e VictoriaMetrics | `GET /metrics` local e Prometheus Remote Write opcional. |
| Escopo hardware | CPU, RAM, GPU, discos, SMART, energia, rede, sensores | Contratos explícitos para disponibilidade parcial por fabricante/driver/OS. |

### 1.2 Requisitos de versão suportada — baseline inicial, sujeito à homologação

- **macOS:** macOS 15+ em Apple Silicon M1 ou mais recente. Versão mínima é meta de teste, não limitação intrínseca declarada do Slint.
- **Linux:** distribuições desktop atuais (primeira bateria em Ubuntu 24.04 LTS+, Fedora atual, Debian stable e Arch) em x86_64/aarch64, preferencialmente kernel 6.1+. Requisitos de glibc, libudev e bibliotecas gráficas serão fixados pelo build reprodutível e testados por distro.
- **Windows:** Windows 11 x64 suportado e testado; edições Home/Pro primeiro, respeitadas as diferenças de permissão.
- **Windows 10:** backlog explicitamente pós-V1 e **legado**; suporte comum da Microsoft terminou em 14/10/2025. Eventual compatibilidade deve ser rotulada best-effort e não confundir com segurança/suporte do sistema operacional.
- **Windows ARM64:** backlog pós-V1, sem presumir que driver e sensores x64 funcionarão via emulação.
- **Headless:** agente e exportação sem GUI poderão operar em máquinas sem ambiente gráfico. Headless em Mac exige serviço/daemon adequado e consentimento especial, não depende do LaunchAgent do usuário.

### 1.3 Não objetivos iniciais

- Rainmeter, skins/widgets presos ao desktop, hardware tuning/overclock, fan control, controle de tensão/potência, monitoramento de hosts remotos por tela, SaaS, contas, login, sincronização de dados, login social e painel web.
- Driver de kernel próprio, extensões kernel não assinadas, desabilitar proteções do sistema, usar APIs privadas como dependência obrigatória ou manter daemon privilegiado executando a coleta inteira.
- Prometer watts totais da tomada, energia por processo ou vida útil exata do SSD quando o hardware não fornece tais dados.
- Substituir ferramentas forenses, EDR, antivírus ou backup.

## 2. Produto, usuários e jornadas

**Posicionamento:** monitor de sistema em primeiro lugar; gerenciamento básico de processos e diagnóstico de gargalos em segundo; histórico e integração com observabilidade como recursos avançados.

**Personas:** (a) usuário avançado identificando processo pesado; (b) desenvolvedor investigando CPU, memória, GPU, I/O e energia; (c) operador/SRE correlacionando workstation e VictoriaMetrics; (d) entusiasta acompanhando sensores e desgaste de SSD; (e) usuário de notebook investigando autonomia/bateria.

**Jornadas com critério mensurável:**

1. Abrir o app pelo ícone, sem comando de terminal ou inicialização manual do agente, e em até 2 interações encontrar o processo que mais consome CPU ou RAM; ordenar por coluna e filtrar por nome/PID. Se o agente falhar, a GUI explica o estado e oferece recuperação/diagnóstico.
2. Ver CPU total e por núcleo, memória, GPU e I/O com séries temporais e eixo temporal compartilhado.
3. Selecionar disco físico → observar IOPS/MB/s/latência/temperatura → abrir aba SMART → entender diferença entre ocupação do volume e condição da unidade.
4. Selecionar GPU entre duas ou mais → utilização, VRAM, temperatura, frequência, potência, processos quando suportados; ver claramente campos indisponíveis.
5. Selecionar processo → visualizar métricas, árvore, detalhes, iniciar encerramento gentil e só então forçado com confirmação e checagens.
6. Fechar GUI → agente continua amostrando e enviando Remote Write → reabrir GUI → ver histórico recente e estado de exportação.
7. Instalar sem administrador quando possível → recursos básicos operam → solicitar autorização apenas ao abrir recurso restrito → revogar/desinstalar helper sem quebrar funcionalidades públicas.
8. Configurar Prometheus local em `127.0.0.1` ou Remote Write via HTTPS para VictoriaMetrics → validar no destino com métrica conhecida, status e contadores de perda.
9. Receber alerta de aquecimento, armazenamento perto do limite ou desgaste/erro reportado → abrir evento com dados que originaram a condição.
10. Executar `monitorctl doctor` → visualizar capacidades e ausência de permissões sem recolher dados privados ou publicá-los involuntariamente.

## 3. Diretrizes de design e UX

### 3.1 Filosofia visual

- **Clareza de Windows 7 + acabamento moderno**, não um clone pixel a pixel. Informação prioritária, poucos cliques, densidade controlada, nenhuma dependência de efeitos para transmitir dados.
- **Desktop-first:** suporte a teclado, mouse, touchpad, redimensionamento, múltiplos monitores e HiDPI; mínimo funcional sugerido 960 × 640 pontos lógicos, conforto a partir de 1200 × 760.
- **Interface nativa sem WebView:** usar Slint + componentes próprios quando necessário, mantendo semântica de acessibilidade e navegação.
- **Padrão de navegação:** sidebar compacta no desktop grande; modo compacto de ícones em janelas estreitas; detalhes em painel adjacente ou aba contextual, não modais sucessivos.
- **Densidade configurável:** confortável, compacta e extra compacta. Preferência persistida por usuário.
- **Temas:** claro, escuro, seguir sistema e alto contraste. Nunca codificar estado apenas por cor.
- **Sem elementos decorativos permanentes:** gráficos, números e tabelas são a interface. Evitar cartões de altura excessiva.

### 3.2 Design tokens sugeridos — base inicial, não identidade de marca definitiva

| Token | Valor / regra |
|---|---|
| Espaçamento | Grade de 4 px; espaçamentos usuais 4/8/12/16/24 px. |
| Tipografia | Fonte de UI da plataforma (SF no macOS; Segoe UI no Windows; sans padrão no Linux), fallback consistente. |
| Texto | 12–13 px tabelas compactas; 14 px corpo; 18–22 px títulos de painel; 26–32 px métrica principal. |
| Números | Algarismos tabulares, unidades semânticas, números alinhados à direita; opção monoespaçada para PID e códigos. |
| Linhas de tabela | 28 px compacta; 36 px confortável; altura configurável. |
| Cantos | 4–8 px nas superfícies; evitar arredondamento excessivo. |
| Bordas | Sutis e úteis na separação; sombras mínimas. |
| Animações | Só transições funcionais 100–180 ms; respeitar preferência de movimento reduzido. |
| Cores de séries | Paleta acessível para CPU/memória/disco/rede/GPU; diferenciação por estilo e rótulos, não apenas matiz. |
| Estados | Normal, aviso, crítico, dado antigo, indisponível, sem permissão, erro; legenda explícita. |

**Exemplo de paleta de referência dark:** fundo `#11161D`; painel `#19212B`; divisória `#2C3744`; texto primário `#EDF2F7`; secundário `#A6B3C2`; seleção `#25405B`; foco `#7DB6FF`; sucesso `#4FC59A`; aviso `#E8B75B`; erro `#F17E7E`. No light: fundo `#F5F7F9`, painel `#FFFFFF`, texto `#15202B`, divisória `#D7DFE7`. Contraste AA deve ser medido, e a paleta final ajustada pelos testes.

### 3.3 Layout proposto

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Monitor do Sistema    [Host atual]       [Pesquisa Ctrl/Cmd+K]  [⚙] [⋯]      │
├───────────────────┬──────────────────────────────────────────────────────────┤
│ Visão geral       │ CPU  12%   Memória  48%  GPU  8%  Disco  3%              │
│ Processos         ├──────────────────────────────────────────────────────────┤
│ Desempenho        │ Gráfico histórico principal / seleção de período        │
│  ├ CPU            │                                                          │
│  ├ Memória        ├──────────────────────────────────────────────────────────┤
│  ├ GPU            │ Processos       [Filtrar] [Colunas] [Agrupar]            │
│  ├ Discos         │ Nome   PID   CPU   RAM   Disco   Rede   GPU   Energia    │
│  ├ Rede           │ ...                                                      │
│  └ Energia        │ ...                                                      │
│ Armazenamento     │                                                          │
│ Hardware          ├──────────────────────────────────────────────────────────┤
│ Alertas           │ Agente ativo · Última coleta há 1 s · Exportação OK     │
│ Exportação        │                                                          │
│ Configurações     │                                                          │
└───────────────────┴──────────────────────────────────────────────────────────┘
```

**Lacunas da tela funcional do M0 (captura macOS):** a captura comprova coleta e conexão, mas ainda é uma tela de validação, não a experiência V1. Antes de expandir para M2, a primeira jornada deve:

- Separar **Visão geral**, **Processos** e **Desempenho** em navegação clara, seguindo o layout proposto, sem empilhar todos os trabalhos numa única tela.
- Fazer os gráficos ocuparem a área de plotagem disponível, com eixo temporal, unidade, seleção de janela de 1 ou 5 minutos e estado de aquecimento/ausência; eliminar séries fixas agrupadas num canto com grande área vazia.
- Dar hierarquia e densidade consistentes a métricas e tabela: colunas alinhadas/ordenáveis, filtro fácil de encontrar, rolagem e layout útil ao redimensionar.
- Exibir estado do agente e idade da última amostra de forma legível; falha, inicialização e dados indisponíveis precisam indicar a próxima ação sem representar ausência como zero.
- Validar a tela em 960 × 640 e 1200 × 760, HiDPI e fontes nativas; manter screenshots de referência por plataforma para revisão visual.

### 3.4 Telas e comportamento

**Visão geral:** resumo de CPU, memória, swap, GPU, disco, rede, energia e sensores; destaque do maior consumidor; indicadores de estado térmico e falhas de coleta. Cada resumo abre detalhe com um clique. Sem inventar total de energia do sistema.

**Processos:** tabela virtualizada para milhares de entradas; nomes, ícone se disponível, PID, PPID, usuário, CPU instantânea e média curta, RSS e percentual, memória compartilhada quando disponível, leitura/escrita de processo, threads, GPU/VRAM e energia **somente se suportados**, status, prioridade e tempo de execução. Árvore expansível, busca incremental, filtros, agrupamento por árvore/usuário, múltiplas colunas e ordenação estável com PID + hora de criação. Não reposicionar descontroladamente as linhas quando usuário está selecionando/inspecionando um processo: congelar ordenação opcional.

**Detalhes do processo:** gráfico de CPU/RAM/I/O, PID e início, caminho/linha de comando com proteção de dados privados, árvore pai/filhos, permissões da ação. Encerrar → confirmar identidade → solicitar término gentil → aguardar prazo → opção forçada. Jamais finalizar por PID sem conferir start time. Ações restritas não ficam disponíveis sem explicação.

**Desempenho:** gráficos CPU por núcleo (núcleos heterogêneos classificados quando OS fornece), memória (sem confundir cache com memória indisponível), swap, páginação, GPU múltipla, disco físico e rede. Intervalos 1m/5m/15m/1h/24h conforme histórico disponível; cruzamento de cursor sincronizado e min/máx/média. Marcar lacunas e reinicializações; não interpolar ausência de dados como zero.

**Armazenamento:** separar claramente **unidade física → partições → volumes/montagens**. Para unidade: modelo, interface, capacidade, temperatura, leitura/gravação MB/s, IOPS, busy time, latência se disponível e alerta SMART/NVMe. Para volume: capacidade total, usada, livre, formato e ponto de montagem. Várias partições de um SSD não significam vários discos. Permitir disco físico sem volume e USB/RAID como disponibilidade limitada.

**Saúde do disco:** duas seções independentes: (1) *estado relatado pelo dispositivo*, SMART/indicadores NVMe/erros, (2) *sinais de desempenho*, latência, retries e I/O. SMART `PASSED` não significa garantia de integridade nem previsão exata de falha. Exibir percentagem de desgaste **somente** se dispositivo expõe atributo interpretável; não inferir “100% saudável” a partir de SMART global OK.

**GPU:** seletor de dispositivo, memória dedicada/compartilhada/unificada discriminada, engine usage (graphics/compute/video quando exposto), frequência, temperatura, watts e limite nominal, ventoinhas e processos. Multi-GPU e hibridação preservam identidades; VRAM compartilhada em SoCs Apple não pode ser apresentada como VRAM dedicada equivalente.

**Energia:** Watts instantâneos de componentes quando medidos/expostos, energia integrada Wh/J quando suportada, estado bateria/tomada, carga e descarga, ciclos e desgaste da bateria, thermal pressure, battery saver e política de baixo consumo. “Energia por processo” somente como medição suportada ou **estimativa identificada**, não converter CPU% arbitrariamente em watts.

**Rede:** upload/download por interface e conexões agregadas; quando disponível, conexão por processo com permissões e sensibilidade adequadas. Não habilitar captura de pacotes. Diferenciar tráfego de interface física, VPN/túnel e loopback; evitar dupla contagem.

**Hardware:** inventário CPU/GPU/armazenamento/sensores/ventoinhas/firmware, identificadores truncados por padrão, driver/fonte e diagnóstico de disponibilidade. Painel `Por que esta métrica não aparece?` com solução específica.

**Alertas:** critérios para saturação sustentada, temperatura, erros de unidade, pouco espaço, falha de coleta/agent/exporter e degradação térmica. Histerese, tempo mínimo, supressão/deduplicação, silêncio configurável e ações locais seguras; sem alertar por picos de um único segundo como padrão.

**Exportação:** cards de conexão Prometheus (pull) e Remote Write (push), teste de conexão, métricas selecionadas, latência da fila, dados perdidos e último envio. Nunca exibir tokens em texto aberto.

**Configurações:** atualização, histórico, idioma, formato decimal/bytes, unidades, privacidade, permissões, início automático, APIs experimentais, exportação, logs, atualização/desinstalação e relatório de diagnóstico.

### 3.5 Teclado, acessibilidade e localização

- `Ctrl/Cmd+F`: filtrar lista; `Ctrl/Cmd+K`: ações e navegação; `Esc`: cancelar; setas/Home/End/PageUp/PageDown: navegação em tabela; `Enter`: detalhes; `Delete`: iniciar fluxo de término com confirmação; `F5`/`Cmd+R`: atualizar; `Space`: pausar/reiniciar gráfico. Atalhos específicos por OS são documentados e remapeáveis no futuro.
- Acessibilidade por voz/leitores de tela, foco visível, ordem lógica, descrições textuais de gráficos, zoom até 200%, não depender de hover; testagem em VoiceOver, NVDA e Orca.
- i18n desde o começo: **pt-BR e en-US na V1**, strings externalizadas, pluralização, separadores e tamanhos de arquivo configuráveis (SI/IEC). Layout suporta textos maiores sem truncar dados críticos.
- Todo estado de métrica deve ter tooltip explicando unidade, definição, origem, instante de coleta e limites de interpretação.

## 4. Arquitetura técnica

### 4.1 Processos locais (separados, não microsserviços)

```text
┌────────────────────────┐                ┌──────────────────────────────┐
│ Desktop GUI — Slint    │◀─ IPC local ─▶│ Agente Rust independente    │
│ Modelos UI / gráficos │                │ Descoberta + agendamento    │
│ Não coleta diretamente│                │ CPU/RAM/GPU/disco/rede      │
└────────────────────────┘                │ Ring buffers / alertas       │
           ▲                               │ Prometheus / Remote Write   │
           │ IPC reutilizado              └────────────┬─────────────────┘
┌──────────┴─────────────┐                             │ autorização / RPC
│ monitorctl CLI        │                             ▼
│ doctor, status, probe │                ┌──────────────────────────────┐
└────────────────────────┘                │ Helper privilegiado opcional│
                                         │ SMART restrito / ações      │
                                         │ Permissões estritas / ACL   │
                                         └──────────────────────────────┘
```

**Regra:** coletar UMA vez por cadência no agente; não iniciar outro ciclo quando UI, CLI e exporter estiverem conectados. UI não bloqueia thread principal com I/O, FFI ou rede. Exportadores consomem snapshots, não acionam leituras independentes.

**Modos de execução:**
1. **Modo usuário padrão:** o agente inicia no login e continua após fechamento da GUI. No macOS usa LaunchAgent, no Linux systemd --user (fallback explícito para outros init), no Windows uma tarefa de logon no Agendador de Tarefas inicia o agente sob a conta do próprio usuário, independente da GUI (sem serviço de sistema por padrão). O comportamento após **logout** não é garantido nesse modo; a UI explica isso.
2. **Modo sistema opt-in:** instala serviço do sistema para headless/sem login (Windows Service preferencialmente `LocalService`, `systemd` system service ou LaunchDaemon), somente após consentimento/admin e com configuração/IPC segregados por usuário. Agente ainda opera com privilégios mínimos.
3. **Helper privilegiado:** executável diferente, chamado apenas para operações autorizadas, nunca usando privilégio elevado como transporte de todas as métricas.

**Contrato de primeira execução:** abrir a GUI não exige que a pessoa rode `agent run`, `monitorctl` ou comandos equivalentes. Se o agente ainda não estiver conectado, a GUI solicita ao módulo de ciclo de vida que registre e inicie o agente pelo gerenciador da sessão do usuário; se já estiver ativo, registra apenas o início nos próximos logins. O gerenciador é dono do processo: a GUI não mantém um processo filho nem encerra o agente ao fechar a janela. Estado, erro e retry ficam visíveis. Instalação/atualização em caminho estável e remoção do registro na desinstalação são responsabilidade do empacotamento futuro. Ver ADR-004.

### 4.2 Organização do repositório

```text
native-system-monitor/
├── apps/
│   ├── desktop/                  # GUI Rust + Slint
│   ├── agent/                    # background runtime
│   ├── privileged-helper/       # operações restritas
│   └── monitorctl/              # CLI, diagnósticos e automação
├── crates/
│   ├── domain/                   # ids, MetricSample, Device, ProcessKey
│   ├── collector-api/            # traits, capabilities e sampling policy
│   ├── collectors-common/        # sysinfo e normalização
│   ├── collectors-macos/         # Mach, libproc, IOKit, Disk Arbitration
│   ├── collectors-linux/         # procfs, sysfs, udev, statvfs, hwmon
│   ├── collectors-windows/       # Win32, PDH, WMI/Storage, ETW opcional
│   ├── gpu-nvidia/               # NVML (dynamic loading)
│   ├── gpu-amd/                  # ADLX Windows, AMD SMI/sysfs Linux
│   ├── gpu-intel/                # IGCL / DRM/sysfs
│   ├── gpu-apple/                # capacidades Apple Silicon
│   ├── storage-health/           # SMART, NVMe, bridges e normalização
│   ├── energy/                   # watts/joules, proveniência
│   ├── process-control/          # políticas de ações e PID identity
│   ├── history/                  # ring buffers e rollups
│   ├── alerting/                 # regras/histerese
│   ├── ipc/                      # contratos protobuf e autenticação local
│   ├── exporters/                # OpenMetrics/Prometheus, remote write
│   ├── security/                 # allowlists, auth, keychain
│   ├── settings/                 # TOML, migrações e versões
│   ├── diagnostics/              # doctor/estado dos coletores
│   └── testing/                  # fixtures, clocks e fake collectors
├── ui/                           # .slint, tokens, ícones, traduções
├── proto/                        # contratos públicos e fixtures de versão
├── packaging/                    # launchd/systemd/windows/installer
├── benchmarks/                   # cenários e relatórios comparáveis
├── tests/                        # integração, e2e e hardware real
├── docs/                         # arquitetura, UX, API, security, ADRs
├── .github/workflows/            # CI/release/security
├── Cargo.toml
├── Cargo.lock
├── LICENSE
└── README.md
```

**Convenção package-by-feature:** cada módulo contém modelo, coleta, normalização, testes e documentação; evitar `utils/` e `services/` genéricos sem fronteiras. Adaptadores OS isolam `unsafe` em bordas auditáveis. `tokio` só onde a concorrência assíncrona traz benefício; coletores bloqueantes executados em workers limitados.

### 4.3 Proposta de dependências, sujeita a spike inicial

- **Base:** Rust stable, Cargo workspace, `thiserror`/`tracing`, `serde`, `toml`, `tokio`, `sysinfo` para dados comuns; verificar custo, granularidade e permissões antes de adicionar coletores nativos.
- **Interface:** `slint` e componentes personalizados Slint, com ponte Rust para modelos e snapshots imutáveis; medir `StandardTableView` versus tabela virtualizada customizada sob 10.000 processos.
- **Plataformas:** `windows` para Win32/COM, `libc` e bindings de frameworks Darwin, `/proc`/`sysfs` no Linux; bibliotecas externas FFI somente se manutenção e licenças forem satisfatórias.
- **Protocolo:** `prost` com mensagens versionadas sobre Unix Domain Sockets (macOS/Linux) e Named Pipes (Windows); `serde_json` para diagnósticos e configurações, não como stream de métricas principal.
- **Armazenamento:** ring buffer em memória por série; spool pequeno e opcional em disco para exportador; sem PostgreSQL, banco de rede ou Redis.
- **Evitar dependências nativas desnecessárias:** drivers/sensors via bibliotecas instaladas e carregamento dinâmico; falhas degradam apenas o recurso correspondente.

## 5. Contrato canônico de métricas e capacidade

### 5.1 Dados obrigatórios

```rust
// Contrato ilustrativo; campos/versões finais definidos em proto.
struct MetricSample {
    key: MetricKey,                  // namespace + tipo + equipamento
    value: f64,
    unit: MetricUnit,                // PercentRatio, Byte, BytePerSecond, Watt, Joule, Celsius...
    kind: MetricKind,                // Gauge | Counter | DerivedRate
    wall_time_unix_ns: i64,
    monotonic_tick_ns: u64,
    source: SourceId,                // sysinfo, hwmon, nvml, native_x...
    quality: SampleQuality,          // Measured | Derived | Estimated
    status: SampleStatus,            // Ok | Stale | Unsupported | PermissionDenied | ...
    device_id: Option<DeviceId>,
    process_key: Option<ProcessKey>, // pid + start_time + boot_id
}
```

**Obrigatório:** tipo/unidade explícita, timestamp, fonte, qualidade, idade, status, estabilidade do identificador e contexto de permissão. Valores inválidos não viram zero; série interrompida não vira linha reta; contadores que reiniciam são detectados. Taxas são calculadas com relógio monotônico e deltas válidos, incluindo wraparound documentado e reinicialização de dispositivo/driver.

**Status de capacidade:** `supported`, `unsupported`, `permission_required`, `driver_missing`, `device_not_present`, `temporarily_unavailable`, `error`, `stale`. Qualidade: `measured`, `derived_from_counter`, `estimated`. Status não é um valor numérico da série.

**Identificação:** IDs estáveis para CPU e memória por host, GPUs via UUID/LUID/PCI bus quando disponível; discos via caminho persistente/serial **local** (serial não exportado por padrão), volumes via IDs do SO; processos via `(boot_id, pid, start_time)` para evitar reutilização de PID.

**Contrato semântico:** diferenciar memória física, virtual, cache, swap, VRAM dedicada/compartilhada, I/O lógico por processo versus I/O do dispositivo físico, GPU core power versus board power, bateria discharge power versus consumo total AC. As séries não são somadas se compartilham domínios ou puderem superestimar.

### 5.2 Cadência e custo

| Família | Intervalo GUI recomendado | Observação |
|---|---:|---|
| CPU/memória/processos agregados | 1 s (modo leve: 2–5 s) | Atualizar por delta e coletar somente campos ativos. |
| Tráfego de rede/discos | 1 s | Counters nativos; evitar leituras de filesystem por processo repetidas. |
| GPU (utilização/VRAM) | 1–2 s | Uma leitura por dispositivo/driver; nunca uma por widget. |
| Energia GPU/CPU | 2–5 s | Reduzir em bateria/thermal pressure; depende do driver. |
| Temperaturas e ventoinhas | 3–10 s | Evitar sondagens agressivas. |
| Detalhamento de processos | 2–5 s, somente tela visível | Caches e orçamento de leitura. |
| Inventário de hardware | Na inicialização/hotplug | Reavaliar em hotplug/mudança de driver. |
| Saúde SMART/NVMe | Sob demanda ou 10–30 min opt-in | **Não acordar discos HDD ociosos automaticamente**. |
| Exportação | 5–15 s, configurável | Desacoplada do redraw UI e coleta. |

Os intervalos são defaults propostos, sujeitos a benchmarking. Cada coletor declara frequência mínima, custo estimado, permissões, taxa máxima e impacto energético. A GUI renderiza no máximo a frequência útil dos dados; 60 FPS não é requisito para gráficos que mudam a cada segundo.

## 6. Coletores e compatibilidade

**Legenda:** `B` = base esperada sem helper; `C` = condicional a hardware, driver, permissão ou modelo; `E` = experimental ou aproximação claramente marcada; `—` = não prometido na V1. As letras indicam *meta de implementação*, não dados já homologados.

| Capacidades | macOS Apple Silicon | Linux x64/ARM64 | Windows 11 x64 |
|---|:---:|:---:|:---:|
| CPU total e por núcleo | B | B | B |
| Processos, PID, CPU, RSS, árvore | B/C | B/C | B/C |
| Memória total/pressão/swap | B/C | B/C | B/C |
| Disco: capacidade/volumes | B | B | B |
| Disco físico: bytes/s, operações/s | C | B | B |
| Disco físico: latência e fila | C | B/C | B/C |
| Disco: SMART, desgaste, erros, temperatura | C | C | C |
| GPU: inventário e utilização | C | C | C |
| GPU: VRAM/uso memória | C | C | C |
| GPU: watts, clocks, temperatura | C/E | C | C |
| GPU: utilização por processo | —/E | C | C |
| CPU/SoC: watts/energia | C/E | C | C |
| Energia total da tomada (watts reais) | — | — | — |
| Bateria: estado, carga/descarga | B em notebook | C | C |
| Bateria: ciclos/saúde/capacidade | C | C | C |
| Rede por interface | B | B | B |
| Rede por processo/conexão | E | C/E | C/E |
| Sensores/fans/térmico | C | C | C |
| Exportador Prometheus/Remote Write | B | B | B |
| Finalizar processo autorizado | B/C | B/C | B/C |

### 6.1 Coleta multiplataforma comum

`sysinfo` cobre inventário e grande parte de CPU, RAM, processos, interfaces e discos. Cuidado: várias métricas são deltas; precisa aquecer a coleta e respeitar intervalo mínimo. Macros/features do crate, threading e alocações passam por benchmark; ele não é a fonte universal de watts, SMART ou GPU.

### 6.2 macOS Apple Silicon (ARM64 apenas)

- **CPU/processos:** Mach host APIs, `libproc`, `host_statistics64`, `proc_pidinfo` e fonte comum conforme aferição; respeitar TCC/sandbox do OS sem tentar contornar proteções.
- **RAM/pressão:** VM statistics, pages, compressor, pressure signals suportados; nomenclatura não confundir RAM livre com memória disponível.
- **Discos/volumes:** Disk Arbitration/IOKit, `statfs`, contadores de I/O expostos, bridges para SMART conforme interface. `smartctl` opcional e controlado; Apple SSD interno pode não fornecer todos os atributos de desgaste previstos.
- **GPU Apple Silicon:** inventário Metal/IORegistry e métricas do sistema onde documentadas; utilização, clocks e watts condicionais. Não depender de frameworks privados/sem contrato de compatibilidade em uma funcionalidade crítica. `powermetrics` pode servir como modo diagnóstico opcional se permitido/compatível e sob helper autorizado; nunca executar continuamente por padrão sem medir overhead.
- **Energia:** IOPowerSources para bateria e alimentação; estado térmico via API adequada; valores de domínio CPU/GPU/SoC só com fonte auditável. Discharge estimado da bateria **não equivale** à potência AC total do computador.
- **Agent:** LaunchAgent por usuário; possível LaunchDaemon separado para modo headless autorizado. Assinatura/notarização Developer ID; sem App Store.

### 6.3 Linux (x86_64 + aarch64)

- **CPU/processos/memória:** `/proc/stat`, `/proc/[pid]/stat`, `/proc/[pid]/status`, `/proc/[pid]/io`, `/proc/meminfo`, `pressure stall information` quando presente. Não confundir contadores lógicos `/proc/[pid]/io` com bytes físicos do SSD atribuíveis a um processo.
- **Discos:** `/proc/diskstats`, sysfs block, udev/by-id, filesystem `statvfs`, device mapper, dm-crypt/LVM/RAID e NVMe. Relação físico-volume deve mapear múltiplas camadas, incluindo overlays e mounts virtuais; evitar somar I/O em devices empilhados.
- **Saúde:** SMART/NVMe nativo quando possível; adaptador externo `smartctl -j`, whitelist dos dispositivos enumerados e parsing de JSON/erros de acesso.
- **Energia:** `/sys/class/powercap`, RAPL quando presente, `/sys/class/hwmon`, `/sys/class/power_supply`; energia via deltas de microjoules com wraparound. ARM64 pode não ter RAPL nem sensor equivalente.
- **GPU:** NVIDIA NVML; AMD `amdgpu` sysfs + AMD SMI se driver suportado; Intel DRM e IGCL/Level Zero conforme hardware. Device/engine FD accounting só quando driver + kernel + permissões oferecem.
- **Térmico:** `/sys/class/thermal` e hwmon, sem confiança universal em nomes de sensores. Política de descoberta e rótulos versionáveis por dispositivo.
- **Agente:** systemd --user primário, fallback manual/documentado; systemd system service opt-in. PolicyKit/polkit para operações autorizadas onde presente. Interface Wayland/X11 apenas para GUI.

### 6.4 Windows 11 x64

- **CPU/processos:** Win32 e contadores de desempenho/PDH, `GetProcessTimes`, memória e processos conforme acesso, `GetProcessIoCounters` para I/O contabilizado do processo. Lembrar que processo I/O não é sinônimo de bytes físicos.
- **GPU:** contadores nativos Windows para engine/VRAM se driver expõe; NVIDIA NVML; AMD **ADLX** para Radeon quando suportada; Intel IGCL para dados de telemetria. Normalizar dispositivos, engines e memória compartilhada sem somar duplicadamente.
- **Discos:** PDH/contadores físicos, `GetDiskFreeSpaceEx`, Storage Management APIs/`MSFT_StorageReliabilityCounter`, consulta NVMe e SMART quando disponível. Em USB, storage controller e RAID haverá lacunas.
- **Energia/sensores:** APIs de sistema para bateria, thermal e power plans; bibliotecas de fabricante/driver para CPU package watts e GPU board watts. Dados de terceiros sem fonte verificável ficam indisponíveis.
- **Rede:** IP Helper API para interfaces; ETW opcional e controlado para atribuição por processo, sem captura de pacotes por padrão.
- **Serviços/permissões:** tarefa por usuário para agent padrão; Windows Service em `LocalService` somente no modo sistema opt-in, e Named Pipes com ACL; UAC para helper/instalação. Nunca executar todo o agente como LocalSystem sem justificativa e revisão.

### 6.5 GPUs — política por fabricante

| GPU | Adaptador prioritário | Restrições que a UI deve explicar |
|---|---|---|
| NVIDIA Windows/Linux | NVML dinâmica, sem exigir CUDA | watts de placa, GPU processes e VRAM variam por geração, driver, modo MIG/vGPU. |
| AMD Radeon Windows | ADLX (via driver) | métrica e suporte por modelo; validar a licença do SDK e a forma de distribuição. |
| AMD Linux | `amdgpu` sysfs; AMD SMI em hardware suportado | AMD SMI não equivale a suporte universal a todas as Radeon. |
| Intel Windows | IGCL + performance counters Windows | recursos dependem de driver/dispositivo, x64 inicial. |
| Intel Linux | DRM/sysfs; IGCL/Level Zero quando viáveis | engine accounting e power variáveis; não prometer Watts. |
| Apple Silicon | Metal/IOKit e instrumentos auditados | memória unificada, lacunas de watts/utilização e APIs não públicas. |

**Controle de probe:** falha de um backend GPU não impede CPU/RAM nem derruba o agente. Carregamento de bibliotecas por `dlopen`/`LoadLibrary` e assinatura/licença auditadas. Não carregar binários de caminhos controlados por usuários não confiáveis.

## 7. Regras específicas de armazenamento e energia

### 7.1 Hierarquia de armazenamento

`PhysicalDevice -> Namespace/LUN -> Partition -> Volume/Filesystem -> MountPoint` com relações muitos-para-muitos onde necessário. Suportar SSD NVMe/SATA, HDD, USB, RAID e discos virtuais sem fingir métricas não mapeáveis. Escolha correta do denominador: ocupação é *do volume*; saúde/temperatura são *do dispositivo físico*; I/O pode ser observado em múltiplos níveis e não somado sem deduplicação.

**SMART/NVMe:** overall status, horas de uso, power cycles, temperatura, media/data integrity errors, unsafe shutdowns, writes/reads, percentage used, setores realocados/pending e self-test history quando disponíveis. Unidades de Data Units Written NVMe precisam conversão oficial; não atribuir TBW garantido ao SSD sem especificação do fabricante. Exibir raw + interpretação explicada.

**Segurança física:** não executar SMART self-test automático; não acordar HDD em spin-down, não reconfigurar firmware nem controle de ventoinha; leitura sob demanda respeita timeout e cancelamento; falha de disco pode travar consultas, então isolar e limitar tempo.

### 7.2 Contabilidade energética

- Distinguir potência instantânea (W), energia acumulada (J/Wh), energia de domínio (CPU package/GPU board) e bateria (W estimado de descarga).
- Energia por integração: `E(Wh) ≈ Σ (P(W) × Δt(h))`, apenas quando P é medido/derivado de fonte válida; lacunas não extrapoladas silenciosamente.
- Contadores de energia RAPL e de driver têm wrap/reset; usar contador monotônico, domínio e unidade corretos.
- Não somar CPU package + GPU board + SoC sem verificar fronteira de domínio; GPU integrada pode já estar inclusa.
- Etiquetas `MEDIDO`, `DERIVADO`, `ESTIMADO` e `NÃO DISPONÍVEL` disponíveis em UI, CLI e documentação; exportar qualidade via métrica de informação controlada, sem cardinalidade excessiva.
- Nenhum botão de fan curve, overclock, undervolt ou power cap na V1 — **monitorar ≠ modificar hardware**.

## 8. IPC, APIs e CLI

### 8.1 Protocolo IPC local

**Transporte:** Unix Domain Socket em macOS/Linux, Named Pipe com DACL em Windows. Mensagens protobuf (`prost`) prefixadas por tamanho, limite máximo configurado (por padrão 1 MiB por frame), keepalive, idempotência nas configurações, versionamento de schema e desconexão graciosa.

**Autorização:** checagem da identidade do peer (`SO_PEERCRED`/equivalente no Unix onde suportado, SID/token no Windows), file mode e ACL; permissões separadas para leitura normal, operações de processo, alteração de exportação e operação administrativa. Serviços compartilhados devem isolar clientes de usuários distintos.

**RPCs semânticos:**

| RPC | Objetivo | Autorização |
|---|---|---|
| `GetVersion` / `GetHealth` | negociar versão, mostrar agent e backlog | leitura local |
| `GetCapabilities` | suporte/driver/permissões/cadência por métrica | leitura local |
| `ListDevices` / `ListProcesses` | inventário paginado | leitura local, filtragem por política |
| `SubscribeMetrics` | streaming incremental com cursor | leitura local e limites de taxa |
| `GetHistory` | janela temporal e agregação | leitura local |
| `GetSmartReport` | diagnóstico de unidade sob demanda | leitura ou helper autorizado |
| `GetProcessDetail` | informações de processo protegidas | política por processo |
| `RequestProcessAction` | terminar/sinalizar prioridade | autorização reforçada, auditoria |
| `GetExportStatus` / `ConfigureExport` | métricas e destino remoto | admin local / dono da configuração |
| `ConfigureAlerts` | regras locais | dono da configuração |
| `RequestHelperAuthorization` | fluxo UI → sistema → helper | explícita, pontual |
| `GetDiagnostics` | status e razões de lacunas | leitura local, dados sensíveis redigidos |

**Backpressure:** assinaturas possuem buffer limitado por cliente; updates antigos podem ser agregados/substituídos em UI (nunca números adulterados). Cliente lento não bloqueia coleta nem exportação. Definir negociação major/minor e janela de compatibilidade entre versões dos binários instalados.

### 8.2 CLI operacional

```bash
monitorctl status
monitorctl doctor [--format json]
monitorctl capabilities [--format json]
monitorctl devices [--type gpu|disk|network]
monitorctl processes --sort cpu --limit 25
monitorctl metrics --once --format json
monitorctl disk health --device <stable-id>
monitorctl export status
monitorctl export test
monitorctl agent logs --last 100
monitorctl agent install --scope user|system
monitorctl agent uninstall --scope user|system
```

Comandos são propostas de contrato, não código existente. Ações destrutivas exigirão argumentos explícitos e autorização, não podem ser disparadas inadvertidamente por shell completions. `doctor` terá modo offline e relatório `--redact`, nunca coletando senhas/tokens.

## 9. Prometheus, VictoriaMetrics e observabilidade do próprio agente

### 9.1 Modos

1. **Pull:** endpoint Prometheus/OpenMetrics `GET /metrics`, ativado explicitamente em `127.0.0.1:9464` por padrão. Informações sobre disponibilidade dos sensores, saúde do agent e exporters incluídas. Exposição não-local requer configuração explícita, autenticação/TLS ou túnel privado, firewalls e aviso.
2. **Push:** Prometheus Remote Write (baseline v1 com `prometheus.WriteRequest`, protobuf+Snappy); planejar suporte negociado a 2.0 após testes do receptor. Envio HTTPS ao endpoint configurado, por exemplo VictoriaMetrics single-node `/api/v1/write` ou cluster configurado pelo usuário.
3. **Nenhuma exportação:** default absoluto; o monitor permanece local e funcional sem internet.

**Não prometer que um Prometheus server aceita escrita por padrão:** Remote Write é um protocolo de *envio para receptor compatível*; Prometheus como armazenamento receptor requer configuração apropriada. Testar com VictoriaMetrics e receptor Prometheus habilitado em laboratório.

### 9.2 Amostra de nomenclatura (contrato final após revisão)

```text
native_monitor_build_info{version="...",os="..."} 1
native_monitor_collector_up{collector="cpu"} 1
native_monitor_collector_last_success_timestamp_seconds{collector="cpu"} ...
native_monitor_cpu_utilization_ratio{cpu="total"} 0.12
native_monitor_memory_used_bytes 123456
native_monitor_disk_read_bytes_total{device="nvme0"} 987654
native_monitor_disk_io_seconds_total{device="nvme0"} 1234
native_monitor_gpu_utilization_ratio{gpu="gpu0"} 0.37
native_monitor_gpu_power_watts{gpu="gpu0"} 97.2
native_monitor_disk_temperature_celsius{device="nvme0"} 43
native_monitor_export_queue_samples 3
native_monitor_export_dropped_samples_total 0
```

**Regras:** nomes/unidades Prometheus consistentes; gauges para valores instantâneos, counters monotônicos para totalizadores; evitar duplicação de métricas do `node_exporter` por opção de namespace/famílias habilitadas. `gpu0` e `nvme0` são *aliases locais estáveis*: remapeamento documentado em troca de hardware; evitar etiqueta com serial, MAC, caminho privado, nome de usuário, PID por padrão. Exportação detalhada por processo é **opt-in, limitada por top-N**, com cardinalidade e churn explicitados; não exportar linha de comando, arquivos abertos, IP remoto ou argumentos de processos por padrão.

**Resiliência:** fila limitada em memória, spool em disco limitado opcional; por default: memória ≤ 16 MiB e spool persistente ≤ 128 MiB, política de descarte configurada e contador `dropped_samples_total`. Reenvio 5xx e opcional 429 com backoff exponencial+jitter e Retry-After quando aplicável; não retry cego de 4xx inválido. Ordem por série preservada. Não bloquear coleta quando destino cair. Estado UI mostra destino, erro último, idade do envio, backlog, rejeitados, enviados, descartados e limites.

**Credenciais:** Keychain macOS, secret store do Windows (Credential Manager/DPAPI com escopo apropriado) e Secret Service/libsecret Linux quando disponível; fallback seguro documentado sem gravar tokens em TOML claro. Verificação TLS estrita; proxy e CA customizáveis sem desabilitar verificação por default. Rotação/revogação suportada.

### 9.3 Exemplo de configuração TOML

```toml
[agent]
scope = "user"
start_on_login = true
cpu_interval = "1s"
process_interval = "2s"
slow_sensors_interval = "10s"
history_high_resolution = "2h"
history_rollup = "24h"

[privacy]
export_enabled = false
include_process_metrics = false
include_device_serials = false

[export.prometheus]
enabled = false
listen = "127.0.0.1:9464"

[export.remote_write]
enabled = false
url = "https://vm.example.net/api/v1/write"
protocol = "v1"
interval = "10s"
credential_ref = "keychain:native-monitor/victoriametrics"
max_memory_queue_mib = 16
spool_enabled = false
max_spool_mib = 128

[storage_health]
automatic_scan = false
avoid_waking_sleeping_disks = true

[ui]
theme = "system"
density = "compact"
language = "pt-BR"
```

Este TOML é **exemplo de contrato de configuração**, não configuração validada por uma implementação. Parser rejeita chaves incorretas relevantes e apresenta mensagens de migração, sem perder dados. Alterações sensíveis exigem validação e confirmação.

## 10. Segurança, privacidade e modelo de permissões

### 10.1 Fronteiras de confiança

- GUI não é confiável para autorizar operações privilegiadas; validar autenticação, parâmetro, owner e capacidade no agente/helper.
- Helper restrito a comandos allowlisted (ex.: leitura SMART de ID de disco validado; finalizar processo somente quando política/usuário permitirem). **Sem comando shell arbitrário, caminho fornecido pelo cliente, flags arbitrárias de `smartctl`, escrita em dispositivos ou mudança de firmware.**
- Acesso a ações administrativas depende do sistema e é registrado localmente com hora/autor/tipo/status, sem argumentos secretos.
- Se `smartctl` for executável externo, resolver binário confiável em caminho fixo/verificado, passar argumentos como vetor, validar ID de dispositivo, timeout, cap de saída e sandbox quando viável.
- Windows: conta de serviço mínima, DACL de Named Pipe, UAC, validação de SID/usuário; impedir que usuário sem privilégios consulte dados privados de outras sessões.
- Linux: UDS com mode 0600 e diretório dono do usuário, polkit para exceções, capabilities granulares se estritamente necessárias. Sem setuid binário genérico.
- macOS: launchd, Authorization Services/SMAppService ou mecanismo Apple suportado conforme validação de versão, assinaturas de binários, verificação de origem dos helpers e permissões. Não contornar SIP/TCC.
- Configuração de exportação é opt-in; endpoint de pull restrito ao loopback por padrão. Não construir um servidor HTTP de administração aberto em `0.0.0.0`.

### 10.2 Dados potencialmente sensíveis

Nomes/caminhos/argumentos de processos, nomes de rede, IPs, serial de disco, hostname e usernames podem identificar pessoas e segredos. Na UI exibir conforme permissão; em logs/redes mascarar por padrão; no relatório de suporte pedir revisão prévia dos campos. *Não fazer captura de tráfego nem armazenar comandos privados.* Nenhum analytics de produto ou telemetria de uso habilitada por padrão.

### 10.3 Supply chain e auditoria

- Inventário de dependências, SBOM CycloneDX ou SPDX, `cargo audit`/alternativa mantida, license scanner, advisories e revisão de FFI.
- Releases versionadas com tags assinadas onde possível, checksum SHA256 e proveniência verificável, CI segregada de segredos de assinatura.
- Análise de threat model STRIDE dos três canais (GUI↔agente, agente↔helper, agente↔destino remoto); fuzz de parser IPC/config/SMART JSON.
- Relatório de vulnerabilidade `SECURITY.md`, SLA de triagem da comunidade e política de atualização segura.
- Instalação, atualização e desinstalação garantem limpeza de serviços e helper, sem deixar auto-start órfão, tokens em logs nem serviço privilegiado antigo.

## 11. Histórico, estado e concorrência

- Ring buffer para séries rápidas com meta de 2h a 1s; agregados 1m até 24h em memória com limites calculados por número real de séries; retenção adaptativa e limites explícitos. Após reboot só há dados preservados se persistência local estiver habilitada.
- Instância única do agente por escopo; GUI multi-instância pode conectar sem duplicar coletas.
- Descoberta periódica de hotplug (USB SSD, GPU externa onde OS suportar, interfaces de rede); remover/degradar dispositivos sem travar.
- Isolar coletores potencialmente bloqueantes em workers controlados, com timeout/circuit breaker; falha da API NVIDIA não derruba painel de processos.
- Pausar/desacelerar sondagens caras ao receber eventos de bateria baixa/thermal pressure, deixando explícito quando frequência muda.
- Usar relógio monotônico para deltas e wall clock para relatório; alterações de timezone/NTP não geram throughput absurdo nem amostras invertidas.
- Tratar sleep/wake, hibernação, troca de usuário, logout, reinicialização do serviço e atualização de driver como eventos de descontinuidade.

## 12. Plano de implementação em incrementos verticais

> **Regra de execução:** cada incremento deve deixar uma jornada funcionando de ponta a ponta (coletor → domínio → agente → IPC → UI → teste → documentação). Não terminar “todos os coletores” antes de construir a primeira interface usável.

### Acompanhamento de implementação

**Atualizado em 2026-10-26.** Estados abaixo refletem evidências presentes no
repositório; CI verde não substitui ensaio visual, hardware real ou critérios de
aceite que ainda não foram executados.

| Incremento | Estado | Evidência / pendência principal |
|---|---|---|
| M0 — Fundamentos executáveis e validação Slint | Parcial | Workspace, GUI/agent/CLI, IPC, ADRs e CI Linux/macOS/Windows existem. Spikes de acessibilidade e renderer alternativo, screenshots por OS e validação nativa completa continuam pendentes. |
| M1 — Processos, CPU e memória | Parcial | Coleta, IPC, busca/ordenação, gráficos, ciclo de vida por usuário, identidade PID + início e seleção de detalhes implementados; testes headless e CI multiplataforma passam. O teste release de 10k processos mediu query + protobuf p95 de 0,97 ms (máx. 1,46 ms) neste host; isso não mede latência/renderização da UI. Faltam E2E/visuais e budget de interação nativa por OS. |
| M2 — Desempenho, disco e rede | Em andamento | Publicado em `786314d` (volumes + taxas RX/TX), `2719eb7`/`b60444b`/`0c73bb4` (I/O físico Linux e correções multiplataforma), `5fa373f` (I/O por filesystem/volume) e `3725531` (latência média Linux por deltas de `/proc/diskstats`, IPC, GUI/CLI); CI `40289499003` passou em Linux, macOS e Windows. I/O/IOPS/busy-time/latência Linux seguem separados dos volumes. `scripts/validate-linux-block-io.sh` compara taxas do agente com amostras independentes e timestampadas de `/sys/class/block/*/stat`, em carga opt-in limitada a 256 MiB, com arquivo temporário limpo por trap. Smoke local de 16 MiB em Btrfs (`/dev/sda1`) observou uma amostra CLI de ~16,2 MiB/s e delta sysfs de 33.032 setores (~16,1 MiB) no dispositivo `sda` durante a janela próxima; spot-check de direção/ordem de grandeza, não taxa precisamente alinhada nem benchmark de capacidade (há I/O concorrente e camadas/filesystems). Um segundo ensaio de 8 MiB confirmou execução e limpeza. Hotplug real, validação semântica por OS e adaptadores nativos macOS/Windows seguem pendentes. |
| M3 — GPUs, VRAM e sensores | Parcial | Publicado em `6bd63bb`, CI `40304637425` passou em Linux/macOS/Windows. Primeira fatia Linux/NVIDIA implementada com NVML dinâmico opcional; UUID, modelo, utilização, VRAM, temperatura, clocks e potência opcional percorrem domínio, IPC, GUI e CLI (`devices --type gpu`). O collector detecta driver/hardware ausente sem derrubar o agente e mantém campos sem suporte ausentes. Teste de collector e smoke agente→IPC→CLI foram executados neste host com NVIDIA GeForce MX230; validação visual e outros modelos pendentes. AMD, Intel, Windows e Apple Silicon seguem sem adapters. |
| M4 — Saúde do disco, energia e bateria | Parcial | Primeira fatia Linux somente leitura de bateria em `/sys/class/power_supply`: estado, capacidade, energia, potência, temperatura e ciclos opcionais percorrem domínio, IPC, GUI e CLI (`devices --type battery`). Fixtures cobrem unidades, ausência e dados inválidos; smoke agente→IPC→CLI enumerou uma bateria real, mantendo temperatura não fornecida como ausente. SMART/NVMe, adapters macOS/Windows, alertas/histerese e validação em mais hardware seguem pendentes. |
| M5 — Prometheus, VictoriaMetrics e histórico avançado | Parcial | Pull OpenMetrics 1.0 em `crates/exporters`: `/metrics` loopback opt-in, snapshot existente, aliases sem dados privados, campos ausentes omitidos, allowlist tipada e métricas de saúde. Scrapes concorrentes (4), cabeçalho (4 KiB), timeout (2 s) e séries (128 dispositivos, 512 CPUs) limitados; CI `40318138924` passou em Linux/macOS/Windows para a versão anterior do exporter. Remote Write v1 codifica protobuf+Snappy e envia snapshots via cliente HTTPS sem redirects; opt-in, HTTP só em loopback, intervalos e fila validados, fila FIFO em memória bounded, backoff/jitter e Retry-After delta-seconds limitado; erros permanentes 4xx descartam lote. `/metrics` expõe enabled, fila (lotes/bytes), sucessos, retries, rejeições, descartes e timestamp do último sucesso; contadores usam OpenMetrics `counter` e os valores são voláteis. Gate local completo para `253cae9` passou (fmt, Clippy, testes workspace, build e laboratório/config); CI `40384814605` passou. A fatia de métricas de fila/status em `78afbe0` passou o gate local completo e CI `40385844421` passou; atualização documental `372f399` também passou no CI `40386053918`. Laboratórios isolados confirmaram `native_monitor_cpu_utilization_ratio` via API de séries VictoriaMetrics v1.154.0 e via API de consulta Prometheus v3.15.1 com `--web.enable-remote-write-receiver`; container/rede próprios removidos. O novo laboratório Prometheus em `2dc655a` passou CI `40389053378` em Linux/macOS/Windows, além de passar localmente. Permanecem credential stores/rotação/revogação, spool durável e destinos autenticados. ADR-010 e ADR-011. |
| M6 — Gestão avançada de processos | Em andamento | Primeira fatia somente de leitura definida em ADR-012: operação IPC minor 9 consulta uma árvore num snapshot por identidade PID + hora de início; BFS mantém raiz primeiro e limite configurado, stale identity retorna `process_not_found`, truncamento é explícito, e CLI `process-tree PID:START_UNIX_SECONDS` imprime a hierarquia. Testes IPC cobrem descendentes, PID reutilizado e truncamento. Gate local completo passou (fmt, Clippy workspace, todos os testes workspace; 1 teste manual de performance ignorado; build workspace). Smoke agente→IPC→CLI real passou para PID 1, incluindo identidade exata e árvore limitada; CI `40387950029` passou em Linux, macOS e Windows. Nenhum sinal, elevação de privilégio ou exposição de command line/caminho é incluído; fluxos destrutivos continuam pendentes de decisão e validação por OS. |
| M7 — Design, acessibilidade e estabilidade V1 | Em andamento | Primeira fatia de semântica Slint adiciona tab-list para navegação, tabs com estado selecionado, ações acessíveis em navegação/faixas/ordenação/linhas de processos, label explícito no filtro, role list-item e anúncio polite do status do agente. Contraste medido no texto lateral pequeno: `#718197`/`#171e27` era 4,23:1; substituído por `#8292a5`/`#171e27` em 5,28:1. Testes desktop com `SLINT_EMIT_DEBUG_INFO=1` passaram antes dessa correção de cor; gate completo será repetido. Isso não comprova operação por teclado nem suporte de leitor de tela; auditoria de todas as combinações, validação nativa por OS, i18n e soak permanecem pendentes. |
| M8 — Release público e manutenção | Em andamento | `CONTRIBUTING.md` registra toolchain e gate exatos do CI, layout, ensaio de performance opt-in, laboratórios locais e limites de segurança. Ainda faltam política/canal de divulgação `SECURITY.md`, licença e decisões de release, empacotamento, assinatura/notarização, SBOM verificável, checksums/proveniência e homologação de instalação/upgrade/desinstalação por OS. |

**Sequência de execução:** fechar lacunas nativas de M0/M1 quando houver
ambiente/plataforma disponível; concluir a jornada M2 (I/O físico, validação de
rede e disco com fonte independente e hotplug) antes de declarar o incremento
pronto. Depois avançar M3
→ M8 em fatias completas e testáveis, sem marcar como homologadas capacidades
que dependem de hardware ou de uma sessão desktop ausente. Atualizar esta tabela
a cada marco verificado.

### M0 — Fundamentos executáveis e validação Slint

**Entregas:** Cargo workspace, `apps/desktop`, `apps/agent`, `monitorctl`, CI Windows/macOS/Linux, temas básicos Slint, IPC mínimo, logging estruturado, exemplo de fake collector, documentação ADR-001 (Slint), ADR-002 (separação de processos), definição de licenciamento e matriz de build.

**Spikes obrigatórios:** tabela 10k linhas com ordenação/filtro, gráfico 1200 amostras, acessibilidade por OS e fallback renderer GPU/software. Reavaliar GPUI/Iced **apenas se** Slint falhar gates mínimos definidos; registrar números, não decidir pela estética apenas.

**Aceite:** abrir GUI nos três OS V1; agente persistir após fechamento GUI; CLI mostrar status; build reproduzível; sem root/admin por padrão; testes de IPC e fake data passando. O M0 é um slice de desenvolvimento: iniciar o agente manualmente por terminal é permitido somente nesse fluxo e não define a experiência do produto instalado.

### M1 — Primeira jornada completa: processos + CPU + memória

**Entregas:** CPU total/por núcleo, RAM/swap, lista de processos, PID/start-time, top CPU/RAM, busca, ordenação, seleção e gráfico 5min, histogramas quando necessários, estados de permissão, testes e benchmarking inicial; platform adapters mínimos. Incluir registro/inicialização do agente no login por usuário (LaunchAgent, systemd --user e tarefa de logon), primeira abertura sem terminal e recuperação/diagnóstico pela GUI. Revisar a tela M0 para cumprir os critérios visuais acima.

**Aceite:** nenhum valor fake em modo real; processo criado/encerrado aparece e desaparece corretamente; PID reutilizado não herda histórico; abrir a GUI numa instalação limpa conecta ao agente sem comando manual; agente continua após fechar a GUI; há no máximo uma instância por usuário; falha de inicialização tem recuperação/diagnóstico pela GUI; comportamento com 10k processos sintéticos passa orçamento de interação. Screenshots e testes de redimensionamento confirmam os critérios de UI em cada OS V1. A remoção automática de registros no uninstall depende do empacotamento de M8.

### M2 — Desempenho, disco e rede

**Entregas:** `/proc/diskstats`/PDH/IOKit conforme OS, físicas/volumes, busy%, IOPS, throughput, latência disponível, rede por interface, `statfs`, múltiplos volumes, hotplug, gráficos detalhados, CLI devices.

**Aceite:** testes com carga de disco e rede verificam tendência e unidade contra fonte independente; partições não duplicam disco; interface virtual não é contada como NIC física; cenários de sem permissão documentados.

### M3 — GPUs, VRAM e sensores por fabricante

**Entregas:** descoberta e enumeração multi-GPU, NVIDIA NVML, AMD Linux/Windows, Intel Linux/Windows, Apple Silicon quando fonte válida, GPU clocks, temperatura, memória e watts condicionais; devices capability matrix específica; fallback por backend.

**Aceite:** cada backend presente e validado em pelo menos um hardware real da sua família antes de marcar suportado; ausência de driver não derruba agente; múltiplas GPUs têm IDs distintos; watts ausentes exibidos como ausentes, não 0 W.

### M4 — Saúde do disco, energia e bateria

**Entregas:** SMART/NVMe opcional, adaptador seguro `smartctl`, desgaste, erros, horas, temperatura, power & thermal states, ciclos da bateria, contadores joules/watts, alertas críticos com histerese, helper mínimo.

**Aceite:** não provocar wake repetitivo do HDD em default; SMART disponível com permissão explícita; sem percentagem de saúde inventada; watts têm etiqueta de fonte e fronteira; operações privilegiadas logadas e negadas se não autorizadas.

### M5 — Prometheus, VictoriaMetrics e histórico avançado

**Entregas:** `/metrics` loopback opt-in, Remote Write v1 com protobuf/Snappy, seleção famílias, fila limitada, reenvio, status, secrets stores, métricas do exporter, manual Prometheus/VictoriaMetrics e laboratório Docker Compose.

**Aceite:** scrape apresenta métricas padrão válidas; VictoriaMetrics ingere série do host; queda temporária do endpoint não interfere na GUI; backlog/drop auditáveis; desligado por padrão; testes contra cardinalidade e vazamento de segredos.

### M6 — Gestão avançada de processos e operação

**Entregas:** processo detalhado, pai/filhos, sinais/encerrar gentil/forçar com confirmação, permissão por usuário, prioridades quando suportadas, logs de auditoria, setup e revogação do helper. Instalação e ciclo de vida padrão do agente passam a ser entregues no M1; este incremento cobre apenas políticas/serviços especiais que dependam de operação avançada.

**Aceite:** proteção contra PID reuse, usuário não mata processos alheios sem autorização do sistema, agent não ganha privilégio indevido, desinstalação remove service/helper e mantém opção explícita de apagar configurações.

### M7 — Design, acessibilidade e estabilidade V1

**Entregas:** polimento visual, density switch, i18n pt-BR/en-US, leitores de tela e teclado, gráfico acessível, tratamento de erros, onboarding não intrusivo, testes de update/reboot/sleep, hardening, perf budget.

**Aceite:** grade UX definida, testes E2E e acessibilidade, 24–72 h soak conforme máquina real; escape de renderer, crash recovery, logs/sanitização e releases candidatas com changelog.

### M8 — Release público e manutenção

**Entregas:** installers assinados quando habilitados, macOS Developer ID e notarização, Windows installer e assinatura se disponível, Linux pacotes, docs, CONTRIBUTING, SECURITY, SBOM, changelog, checksums, matriz de hardwares testados, screenshots reais e benchmark publicado.

**Aceite:** instalação limpa e upgrade/desinstalação nas plataformas V1; nenhum sensor crítico se declara suportado sem ensaio em hardware real; release não requer credencial externa para rodar; hashes/proveniência disponíveis.

### Pós-V1 (priorização por demanda e capacidade técnica)

- Compatibilidade Windows 10 legado e Windows ARM64.
- eBPF/ETW enriquecido para processos e conexões, com revisão de custo/privacidade.
- Históricos locais persistentes opcionais e export CSV/JSON; dados de 30 dias e replay.
- Perfis de amostragem, atalhos customizáveis, notificações inteligentes e plugins de coletores **somente assinados/limitados**.
- Exportação OTLP Metrics opcional, além de Prometheus.
- Dashboard remoto apenas como projeto distinto ou módulo opt-in, sem serviço público implícito.
- Sensores adicionais de placas-mãe, UPS, ventoinhas, BMC e detalhamento de tráfego; controle de hardware continua fora do escopo por padrão.

## 13. Requisitos não funcionais e benchmarks

**Todos os limites abaixo são *targets provisórios de aceitação*, não medições já feitas.** Normalizar uso de CPU por núcleo lógico e publicar metodologia/hardware; registrar memória RSS, PSS onde possível, alocações, disco e wakeups.

| Cenário | Orçamento inicial desejado | Método |
|---|---|---|
| Agente com CPU/RAM/rede/disco básico a 1–2s, sem UI/export | média ≤1% de um núcleo lógico em máquina de referência; RSS ≤80 MiB | medições 30min, 3 plataformas |
| Agente com GPU/sensores ativos | média ≤3% de um núcleo lógico; RSS ≤120 MiB | GPU real, 30min + idle wakeup check |
| GUI aberta ociosa | média ≤2% de um núcleo lógico adicional; RSS ≤180 MiB | 1920×1080 / 100% e HiDPI |
| Primeira janela utilizável | p95 ≤3s em SSD moderno | 30 cold/warm starts por plataforma |
| Filtrar/ordenar 10k processos sintéticos | p95 ≤150 ms até UI responsiva | fixture determinística e profiling |
| Selecionar painel e renderizar 1200 pontos | p95 ≤100 ms de interação; sem freeze >250ms | teste de UI e capture flamegraph |
| Coleta com exporter remoto indisponível | sem crescimento além da fila/spool configurados | 24h falha simulada e retry |
| Rodar 24h de agent e 8h de UI | sem crescimento monotônico significativo de RSS/leaks | soak + hotplug + sleep/wake |
| Startup/resume com GPU driver ausente | restante do sistema continua e explica ausência | injeção de falhas por backend |

Para a consulta M1 de 10 mil processos, o teste manual de release usa fixture
determinística, 10 amostras de aquecimento e 100 medidas de filtragem/ordenação
no agente mais a codificação protobuf dos até 1.000 resultados. A fixture gera
10.000 processos; `process-9` corresponde a 1.111 e a resposta é limitada a 1.000.
Esse número é uma evidência do caminho agente/IPC, não uma medição de pintura da
GUI; o aceite
de p95 até a UI responsiva ainda requer profiling/renderização nativa e os testes
visuais/redimensionáveis previstos para cada OS V1.

**Medição local (2026-10-24):** três execuções release independentes do teste
`server::tests::filter_sort_10k_processes_p95`, cada uma com 10 aquecimentos e
100 amostras. O p95 do caminho agente + codificação protobuf foi,
respectivamente, **0,724 ms**, **0,685 ms** e **0,959 ms**; os máximos por
execução foram 0,868 ms, 0,730 ms e 1,314 ms. Ambiente: Linux x86_64, kernel
7.2.8-2-cachyos, Rust 1.98.1, Intel Core i5-8265U. Isso passa o orçamento do
caminho medido, mas não altera a ressalva acima sobre o p95 end-to-end da UI.

**Comparações:** Windows 11 Task Manager, macOS Activity Monitor e alternativas Linux (`gnome-system-monitor`, `mission-center`, `btop` para cenários representativos). Comparar carga da própria ferramenta sob refresh equivalente; não comparar números de memória com ferramentas que usam definições diferentes sem documentar.

**Cenários de carga:** idle estável, compilação pesada, 10k processos sintéticos, SSD sob fio de leitura/gravação controlado sem desgaste excessivo, tráfego de rede controlado em loopback, GPU benchmark apropriado/curto, troca de bateria/tomada, tela bloqueada, múltiplos monitores, high DPI, hotplug GPU/USB quando possível e simulador de driver ausente.

**Gates de erro sem maquiagem:** CPU e memória com desvio relativo tolerado pela janela de amostragem versus fonte de referência; throughput I/O comparado por mesma janela e denominador; temperatura/potência devem convergir às leituras do vendor após aquecimento, com divergência registrada por dispositivo/driver. Limites finais por métrica só após primeira rodada de calibração.

## 14. Plano de testes

### 14.1 Camadas

- **Unitários:** normalização de unidade, wrap/reset de counter, cálculo de taxa monotônica, agregação, filtros, ordenação, status, regras de alerta, histerese, conversão energia Wh, parsing SMART/NVMe, IDs de devices e processos. Cobertura meta ≥80% nas crates de domínio/transformação; 100% de caminhos de autorização críticos por cenários.
- **Contract tests:** Protobuf IPC vN/vN−1, capabilities, schema settings, version negotiation, limites de frame, autenticação, upgrade de serviço/GUI mistos.
- **Integração OS:** disco virtual versus físico, process churn, processos de outros usuários, interfaces virtuais, suspend/resume, privilégios ausentes, hardwares que negam SMART, driver GPU reiniciando.
- **E2E GUI:** navegação teclado, filtros, colunas, densidade, empty/loading/error states, persistência de preferência, multimonitor, acessibilidade pt-BR/en-US, screen reader.
- **Exportador:** golden fixtures OpenMetrics, protobuf conformance, headers/schemas, VictoriaMetrics de teste, Prometheus scrape, 5xx/429/401/400, TLS inválido, fila cheia, expiração e retomada.
- **Segurança:** DACL/Unix socket identity, read cross-user, comandos helper não allowlisted, TOCTOU PID reuse, injeção de flags/caminho, executável substituído, symlink races, negação de serviço por cliente lento, fuzz IPC e parsing.
- **Distribuição:** assinatura/notarização, revogação de helper, upgrade/rollback, limpeza uninstaller, pacote em máquina limpa sem Rust/toolchain ou driver SDK instalados.

### 14.2 Matriz de hardware real mínima

- Mac Apple Silicon **M1/M2/M3 ou superior**, ao menos um notebook para bateria, um desktop para CPU/GPU e monitor de energia; macOS 15+.
- Linux x86_64 com GPU NVIDIA recente, outro com AMD Radeon, outro com Intel integrada; Linux ARM64 com telemetria básica e sensores condicionalmente disponíveis.
- Windows 11 x64 com NVIDIA, AMD Radeon e Intel integrada em máquinas reais ou uso comunitário documentado; ao menos HDD SATA e SSD NVMe; interfaces USB/RAID em testes exploratórios.
- Sem hardware de uma família, classificar suporte **experimental/não homologado**, ainda que o código compile. Tests de hardware real em runners próprios ou relatórios reprodutíveis da comunidade.

### 14.3 Falhas e regressões obrigatórias

- Corrida de PID e processo protegido; teste de usuário A tentando observar/controlar processo de B.
- `smartctl` com retorno parcial, JSON inválido, drive dormindo, drive desconectado, timeout, disco sem SMART, USB bridge que bloqueia comando.
- NVML indisponível, AMD SMI não instalada, GPU hybrid, device com zero sensores, timeout de driver.
- Dispositivo muda de nome/ordem após reboot; volume renomeado; clock NTP ajustado; máquina hiberna por 4h.
- Remote Write perdendo conexão após envio parcial, 401 por token expirado, 429/Retry-After, spool lotado, destino recebendo duplicatas/reorder.
- Permissão negada por OS, helper não instalado, helper revogado, agent reiniciado durante upgrade, UI de versão anterior.

## 15. Qualidade de engenharia, CI/CD e observabilidade interna

**Qualidade mínima:** `cargo fmt --check`, `cargo clippy` com warnings críticos bloqueantes, `cargo test --workspace`, testes por plataforma, auditoria de dependências/licenças, SAST opcional, docs e contrato. FFI revisado e isolado. Código Rust estrito, `unsafe` justificado com comentários de invariantes, não suprimir warnings de forma global.

**CI:** runners Windows x64, macOS arm64 e Linux x64; Linux arm64 por runner nativo quando disponível; builds e testes a cada PR; E2E Slint/GUI em pipelines apropriadas; nightly com ASan/LSan onde aplicável e benchmarks rastreáveis. Teste de drivers e sensores exige hardware real e é pipeline separado; job que não teve hardware é **skipped**, não `passed`.

**Logs e métricas internas:** trace IDs locais, `collector_up`, latência por coletor, última amostra válida, perf do IPC, tamanho de fila, amostras perdidas, reinícios do agent, overhead da GUI; truncar e redigir dados sensíveis. `monitorctl doctor` gera relatório reproduzível offline; crash reporting remoto não habilitado automaticamente.

**ADRs mínimos:** ADR-001 Slint e renderer; 002 agent/UI/helper; 003 IPC e autenticação; 004 schema de métricas; 005 GPU strategies; 006 storage/SMART; 007 permission model por OS; 008 Remote Write e cardinalidade; 009 histórico e retenção; 010 licença/SDKs; 011 packaging/update; 012 versão mínima de OS.

## 16. Distribuição, licença e comunidade

### 16.1 Open source

- **Proposta:** GPL-3.0-only para o produto, sujeita a revisão jurídica, revisão das licenças de Slint, sysinfo, drivers SDK, smartmontools e bindings FFI. Slint oferece GPLv3 e outras opções; dependências e binários de SDKs de fabricantes não devem ser redistribuídos por inferência.
- Incluir LICENSE, SPDX headers, NOTICE/third-party notices quando aplicável, SBOM e política de contribuição; exemplos e documentação com licenças explicitadas.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, templates Issue/PR, guidelines de hardware report, roadmap público e definição clara de compatibilidade experimental.

### 16.2 Empacotamento por OS

| OS | Artefato V1 | Inicialização do agente | Cuidados |
|---|---|---|---|
| macOS arm64 | `.app` + `.dmg` assinados Developer ID, notarizados | LaunchAgent do usuário; daemon opt-in para system mode | Hardened Runtime, assinaturas dos binários, remover helper no uninstall. |
| Windows 11 x64 | `.msi` via WiX ou instalador equivalente; zip portátil sem serviço como alternativa limitada | Agente via tarefa de logon do usuário; Windows Service LocalService só no modo sistema opt-in | Assinatura Authenticode quando disponível; não instalar serviço em modo portátil sem permissão. |
| Linux x86_64 | `.deb`, `.rpm`, AppImage | systemd --user, helper polkit opcional | deps runtime/versionadas, Wayland/X11, sufixos de pacote e uninstall. |
| Linux aarch64 | `.deb`, `.rpm`, AppImage se homologado | idem | binary/ABI nativo e GPU/sensores explicitamente condicionais. |

**Atualizações:** GitHub Releases assinado/checksum, changelog e notificações opcionais. Updater automático desabilitado por padrão em V1; downloads só via HTTPS e verificação de assinatura/hash; migração de configuração compatível e recuperação/rollback. Instalação headless documentada para usuários técnicos. Sem App Store.

## 17. Backlog estruturado para issues

**Épico E01 — Foundation:** workspace, CI 4 targets, fake collector, versionado IPC, config schema, diagnostics, licensing audit.

**Épico E02 — Process:** CPU, RSS/swap, lifecycle, process tree, table virtualization, sorting/search, pid reuse, details/actions.

**Épico E03 — Performance:** timeseries, rollups, CPU per core, memory pressure, resampling/downsampling, time-cursor crosshair, table-to-graph navigation.

**Épico E04 — Storage:** physical/volume mapper, performance I/O, hotspots, SMART/NVMe parser, privilege-gated health, removable drives.

**Épico E05 — GPU:** capability registry, NVIDIA, AMD, Intel, Apple adapters, VRAM semantics, clocks/thermal/watts, multi-GPU.

**Épico E06 — Power:** battery, package energy counters, thermal, power quality, idle policies, warn/alerts.

**Épico E07 — Network:** interfaces, counters, virtual interfaces, per-process opt-in, errors, sockets.

**Épico E08 — Exporters:** pull, remote write v1, endpoint test, bounded queue, secrets, TLS, metrics hygiene, VM lab.

**Épico E09 — Security & agent lifecycle:** local IPC auth, inicialização automática por usuário sem terminal, recuperação e diagnóstico do agente, instalação/update/uninstall por OS, serviços privilegiados opt-in, helper, ações, auditoria, threat model/fuzz.

**Épico E10 — UX:** navegação por jornada, refinamento da tela validada no M0, gráficos responsivos com eixo temporal, tabela de processos legível/ordenável, estados de startup/erro/permissão, tokens, dark/light, teclado, a11y, pt-BR/en-US, densidade compacta e screenshots de regressão.

**Épico E11 — Release:** build, notarize/sign, packaging, update/uninstall, SBOM, docs, validated hardware matrix and performance report.

**Template de issue:** objetivo, plataforma/dispositivo, fonte de métrica, pré-condições/permissões, UI esperada, payload IPC, limites de privacidade, testes automatizados, teste em hardware real, benchmark delta, documentação e critérios observáveis.

## 18. Critérios de pronto (DoR), concluído (DoD) e gates de release

**DoR de feature:** comportamento definido, valores/semântica/documentação da API, fonte por OS, forma de detectar suporte, fluxo sem permissão, threat impact, plano de benchmark e hardware de teste identificados.

**DoD de feature:** funcionalidade real de ponta a ponta, sem dados mock em release; testes unit/integration/E2E pertinentes; fonte/unidade/qualidade no contrato; erro/stale/unsupported demonstráveis; performance medida; docs/changelog/ADR atualizados; check acessibilidade. Feature pode ter suporte parcial se rotulada como tal e sem quebra das outras áreas.

**Gate V0.1 (foundation):** GUI + agent + IPC + CPU/RAM/processes em três plataformas; persistência após GUI fechar; protótipos UX e testes iniciais.

**Gate V0.5 (hardware alpha):** discos e GPU familiares demonstrados, sensores e energia condicionais, SMART seguro, capability-report completo, sem crashes de backend individual.

**Gate V0.8 (observabilidade beta):** exporter e queue confiáveis, ações seguras, serviço/instalador, i18n, alertas, suporte macOS arm64/Linux x64+arm64/Windows11 x64 com limitações honestas.

**Gate V1.0 (release):** todos os fluxos centrais com testes repetíveis, artefatos assinados onde aplicável, hardware matrix, benchmark real contra referências, security review, filas e export com perda explícita, documentação para instalação/recuperação/desinstalação. Falta de hardware para família implica badge *experimental* ou adiamento de claim; **nunca** “suporte completo” por compilação apenas.

## 19. Riscos e respostas

| Risco | Impacto | Mitigação / decisão |
|---|---|---|
| Slint tabela/gráficos/a11y não atingem metas | Alto | Spike M0 com limites reproduzíveis; renderer fallback; registrar alternativa GPUI/Iced se necessário. |
| Energy/GPU Apple Silicon usa API não documentada | Alto | Não torná-la dependência V1; opção experimental isolada e aprovada; fallback de disponibilidade. |
| Licença AMD/Intel/NVIDIA impede redistribuição | Alto | Dynamic loading/SDK externo quando permitido; revisar os termos antes do merge; não copiar binários. |
| SMART exige root/admin ou acorda discos | Alto | Execução sob demanda, permissão pontual, helper allowlist, timeouts e evitar spin-up. |
| Processo encerrado/privileged escalation mal autorizada | Crítico | PID+start-time, peer identity, ACL, helper restrito, auditoria e testes adversariais. |
| Endpoints remotos e labels vazam dados privados | Alto | Export desabilitado por padrão, secret store, low-cardinality, redaction e loopback. |
| Counters físicos/logical divergem | Alto | Semântica e fonte sempre explícitas; hardware reference tests. |
| UI vira consumidora de CPU/RAM | Alto | Bench gate de consumo, tabela virtualizada, delta updates, sampling e render sob demanda. |
| Compatibilidade Linux por distro/driver | Médio/alto | Artefatos por libc/ABI testados, matriz de distro, degrade parcial e contribuições documentadas. |
| Agente do usuário para em logout | Médio | Explicitar distinção sessão/serviço e oferecer system mode opt-in. |
| Windows 10 pós-EOL | Médio | Não suportar em V1; futura compatibilidade marcada como legado e com riscos. |
| Sistemas protegidos impedem dados de terceiros | Médio | Respeitar restrições; status permission/unsupported; jamais solicitar bypass de proteção. |

## 20. Regras de execução para agentes de desenvolvimento

1. Ler este plano integralmente; registrar ADR quando decisão surgir ou mudar. Não reabrir decisões já aprovadas sem descoberta técnica factual.
2. Implementar verticalmente M0→M8; não fazer benchmark isolado como etapa que bloqueia primeira jornada funcional. Rodar benchmarks desde M1 e atualizá-los a cada marco.
3. Consultar documentação oficial de Slint, sysinfo, OS APIs, fabricantes de GPU/SMART e Prometheus antes de adotar símbolo, crate, API, versão ou comportamento.
4. Usar `cargo fmt`, `clippy`, testes e fixtures; toda mudança de versão de schema deve ter contract test e notas de migração.
5. Nunca declarar hardware, métrica, integração ou plataforma “suportada” sem evidência de teste real. Documentar `not tested` e `unsupported` separadamente.
6. Feature privilegiada ou de exportação exige threat review, limite de cardinalidade e política de privacidade antes de `merge`.
7. Preferir bibliotecas/API nativa confiável a parsing frágil de saída de comandos; quando CLI externa for inevitável, encapsular e testar todos os erros.
8. Não chamar código experimental de medição física definitiva; mostrar qualidade/proveniência no contrato, UI e logs.
9. Produzir ao final de cada marco: arquivos mudados, comandos de build/execução, screenshots reais (quando GUI), resultados dos testes/benchmarks, plataformas/hardwares realmente verificados, decisões e pendências.
10. Não criar conta, backend SaaS ou telemetria externa incidental. O app deve funcionar offline.

## 21. Definição da primeira implementação executável

**Primeiro PR deve conter apenas a primeira fatia vertical completa, e não um skeleton vazio:**

- Workspace Rust com 3 bins (GUI, agent, monitorctl) e crates `domain`, `collector-api`, `collectors-common`, `ipc` e `settings`.
- Agent com start/stop e persistência após fechar a GUI; medição CPU/RAM/processos via `sysinfo`; timeout e status de coleta.
- IPC local e CLI `status`, `capabilities`, `processes`.
- UI Slint com Visualização Geral, tabela de Processos e gráficos CPU/RAM — dados reais e suporte a simulação apenas em testes/preview de design.
- CI básica Linux/macOS arm64/Windows x64; fixtures de hotplug/PID reuse e teste de regressão de performance.
- README com instalação de desenvolvimento e captura de resultado por sistema; documentar diferenças reais encontradas.

**Após esse PR:** evoluir disco/rede (M2), GPU (M3), SMART/energia (M4), exportação (M5), permissões avançadas (M6), polimento (M7) e release (M8), sem reiniciar arquitetura em cada fase.

## 22. Referências técnicas verificadas (fontes oficiais e projetos de referência)

- Slint releases (1.18.1, 21/09/2026): https://github.com/slint-ui/slint/releases
- Slint licença: https://github.com/slint-ui/slint/blob/master/LICENSE.md
- Slint backend winit e renderers: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/
- Slint `StandardTableView`: https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/standardtableview/
- Slint modelos: https://docs.slint.dev/latest/docs/slint/guide/language/coding/repetition-and-data-models/
- sysinfo Rust: https://docs.rs/sysinfo/latest/sysinfo/
- NVIDIA NVML: https://developer.nvidia.com/management-library-nvml
- AMD ADLX: https://github.com/GPUOpen-LibrariesAndSDKs/ADLX
- AMD SMI: https://rocm.docs.amd.com/projects/amdsmi/en/latest/install/install.html
- Intel IGCL: https://github.com/intel/drivers.gpu.control-library
- Linux hwmon: https://github.com/torvalds/linux/blob/master/Documentation/hwmon/sysfs-interface.rst
- Linux powercap: https://www.kernel.org/doc/html/latest/power/powercap/powercap.html
- Smartmontools: https://www.smartmontools.org/static/doxygen/
- Windows Storage Reliability Counters: https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-storagereliabilitycounter
- Windows Process I/O: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getprocessiocounters
- Windows LocalService: https://learn.microsoft.com/en-us/windows/win32/services/localservice-account
- Apple IOPowerSources: https://developer.apple.com/documentation/iokit/iopowersources_h
- Apple distribuição direta e Developer ID: https://developer.apple.com/macos/distribution
- Apple notarização: https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
- Prometheus Remote Write 1.0: https://prometheus.io/docs/specs/prw/remote_write_spec/
- Prometheus Remote Write 2.0: https://prometheus.io/docs/specs/prw/remote_write_spec_2_0/
- VictoriaMetrics Remote Write: https://docs.victoriametrics.com/victoriametrics/integrations/prometheus/
- Fim do suporte Windows 10: https://support.microsoft.com/pt-br/windows/deployment/updates-lifecycle/windows-10-support-has-ended-on-october-14-2025
- Stats (estudo de sensores do macOS, sujeito a licença/APIs): https://github.com/exelban/stats

---

**Observação final:** este é um plano executável e verificável, não uma declaração de que todos os fabricantes e sistemas expõem todas as métricas. A capacidade real será detectada em runtime e homologada por hardware. Evite converter indefinições do fabricante em números fictícios ou promessas de suporte irrestrito.
