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
| Licenciamento/distribuição | **MIT aprovado pelo usuário (2026-11-09)** | Esta decisão substitui a proposta GPL-3.0-only original e alinha-se ao metadado `license = "MIT"` em `Cargo.toml`. O usuário confirmou ser o titular, definiu 2026 e autorizou usar o nome de autoria dos commits (`Lucas Eufrasio`); `LICENSE` foi criado. A allow-list de licenças de terceiros ainda requer revisão e aprovação. Não inferir autorização de redistribuição de SDKs. |
| macOS | **Somente Apple Silicon (arm64)** | Não suportar, compilar, empacotar ou testar macOS Intel. |
| Linux | x86_64 e aarch64 | Wayland e X11 para a GUI, quando presentes; agente independente do display server. |
| Windows V1 | **Windows 11 x64** | Instalador, serviço/agente e coleta nativa Windows. |
| Windows futuro | Windows 10 e Windows ARM64 | Não bloquear a arquitetura; sem promessa de suporte V1. |
| Mac App Store | **Fora do projeto** | Distribuição direta `.app`/DMG sem assinatura/notarização, conforme decisão do usuário em 2026-11-09; verificar e documentar avisos/restrições do Gatekeeper sem contornar proteções do sistema. |
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
- **Densidade configurável:** confortável e compacta estão implementadas com preferência persistida por usuário; a densidade extra compacta segue pendente de definição visual e critérios de UX, sem parâmetros inventados.
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
- **Agent:** LaunchAgent por usuário; possível LaunchDaemon separado para modo headless autorizado. Artefatos V1 sem assinatura/notarização, sem App Store; validar/documentar a experiência de abertura no macOS sem contornar Gatekeeper.

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
| Intel Linux | Adapter experimental Level Zero Sysman opcional (ADR-014); DRM `fdinfo` somente por cliente, não agregado device-wide | engine/memória/power variam por driver e permissão; validar unidade de clock de memória e escopo antes de expor; não prometer Watts sem domínio GPU explícito. |
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
- macOS: launchd, Authorization Services/SMAppService ou mecanismo Apple suportado conforme validação de versão e permissões. A distribuição V1 escolhida é sem assinatura/notarização; se uma futura função privilegiada depender de binários assinados, reabrir a decisão de distribuição antes de implementá-la. Não contornar SIP/TCC.
- Configuração de exportação é opt-in; endpoint de pull restrito ao loopback por padrão. Não construir um servidor HTTP de administração aberto em `0.0.0.0`.

### 10.2 Dados potencialmente sensíveis

Nomes/caminhos/argumentos de processos, nomes de rede, IPs, serial de disco, hostname e usernames podem identificar pessoas e segredos. Na UI exibir conforme permissão; em logs/redes mascarar por padrão; no relatório de suporte pedir revisão prévia dos campos. *Não fazer captura de tráfego nem armazenar comandos privados.* Nenhum analytics de produto ou telemetria de uso habilitada por padrão.

### 10.3 Supply chain e auditoria

- Inventário de dependências, SBOM CycloneDX ou SPDX, `cargo audit`/alternativa mantida, license scanner, advisories e revisão de FFI.
- Releases versionadas com tags assinadas onde possível, checksum SHA256 e proveniência verificável, CI segregada de segredos de assinatura.
- Análise de threat model STRIDE dos três canais (GUI↔agente, agente↔helper, agente↔destino remoto); fuzz de parser IPC/config/SMART JSON.
- Instruções de reporte de vulnerabilidade (`SECURITY.md`) e política de atualização segura. Não presumir canal privado nem SLA: esses compromissos são opcionais e só existem se o projeto os adotar explicitamente.
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

**Atualizado em 2026-11-09.** A CI remota da alteração do gate de licenças, para
`637ed3babdedb63a734f785649a3bbd9b622926a` passou em Ubuntu 24.04, macOS 15 e
Windows 2025, incluindo fmt, Clippy, testes, build e auditoria RustSec; o gate
`cargo deny check licenses` foi instalado e passou no runner Linux.
[Execução 42593553670](https://github.com/lucaswilliameufrasio/native-system-monitor/actions/runs/42593553670).
O commit documental subsequente `27bd5bb` também passou nos três runners
[na execução 42595320776](https://github.com/lucaswilliameufrasio/native-system-monitor/actions/runs/42595320776).
CI verde não substitui ensaio visual, hardware real ou critérios de aceite não
executados.

| Incremento | Estado | Evidência / pendência principal |
|---|---|---|
| M0 — Fundamentos executáveis e validação Slint | Parcial | Workspace, GUI/agent/CLI, IPC, ADRs e CI Linux/macOS/Windows existem. QA virtual em Ubuntu 24.04/Xvfb/Slint software cobriu startup, Storage, navegação/filtro, pt-BR/en-US, SI/IEC e tamanhos 960×640, 1120×720 e 1200×760; um espaçamento visual encontrado foi corrigido com regressão Slint. O renderer `winit-femtovg` iniciou sob Mesa software e a árvore AccessKit foi observada via AT-SPI Linux, sem leitor de tela real ou GPU. O benchmark UI sintético também mediu sort/render de 10k processos e gráficos raw/downsampled abaixo dos budgets no rasterizador Slint software; input-to-frame nativo e flamegraph ainda faltam. Capturas por OS, validação nativa e acessibilidade assistiva completa continuam pendentes. |
| M1 — Processos, CPU e memória | Parcial | Coleta, IPC, busca/ordenação, gráficos, ciclo de vida por usuário, identidade PID + início e seleção de detalhes implementados; testes headless e CI multiplataforma passam. Três execuções release pós-persistência da UI com renderer software (2026-11-08) mediram p95 de sort/render de 10k linhas em 51,669–52,734 ms (máximo 74,420 ms), filtro de 10k para uma linha em 16,749–16,871 ms (máximo 23,521 ms), gráfico raw de 1.200 pontos em 21,062–21,512 ms (máximo 22,264 ms) e downsampled em 16,741–17,334 ms (máximo 17,791 ms). As metas passaram nesse pipeline sintético, não são input-to-frame nativo nem GUI winit real. O benchmark IPC separado de 10k mediu query + protobuf p95 de 1,015/1,158/1,152 ms. QA visual/interativa Ubuntu 24.04 x86_64 verificou um processo sintético aparecer filtrado na GUI, ser encerrado e desaparecer do snapshot e da tela; captura pós-encerramento mostra estado vazio. Encerrar a GUI QA deixou o agente separado ativo (IPC 1.10). Perf/flamegraph, validação nativa por OS e GPU física seguem pendentes. |
| M2 — Desempenho, disco e rede | Em andamento | Publicado em `786314d` (volumes + taxas RX/TX), `2719eb7`/`b60444b`/`0c73bb4` (I/O físico Linux e correções multiplataforma), `5fa373f` (I/O por filesystem/volume) e `3725531` (latência média Linux por deltas de `/proc/diskstats`, IPC, GUI/CLI); CI `40289499003` passou em Linux, macOS e Windows. I/O/IOPS/busy-time/latência Linux seguem separados dos volumes. `scripts/validate-linux-block-io.sh` compara taxas do agente com amostras independentes e timestampadas de `/sys/class/block/*/stat`, em carga opt-in limitada a 256 MiB, com arquivo temporário limpo por trap. Ensaios locais limitados de escrita em Btrfs compararam taxas do agente com deltas independentes de sysfs e confirmaram tendência/unidade em ordem de grandeza; não são taxas precisamente alinhadas nem benchmark de capacidade, devido a I/O concorrente e camadas/filesystems. `scripts/validate-linux-network.py` gera somente tráfego loopback limitado e compara taxas do agente com `/proc/net/dev`; smoke local limitado comparou o tráfego loopback do agente com `/proc/net/dev` e confirmou tendência/unidade em ordem de grandeza, sem constituir benchmark nem validar outras interfaces ou sistemas. CI `40413996153` passou em Linux/macOS/Windows. Isso não é benchmark nem valida outras interfaces/OS. Hotplug real, validação semântica por OS e adaptadores nativos macOS/Windows seguem pendentes. |
| M3 — GPUs, VRAM e sensores | Parcial | Publicado em `6bd63bb`, CI `40304637425` passou em Linux/macOS/Windows. Primeira fatia Linux/NVIDIA implementada com NVML dinâmico opcional; UUID, modelo, utilização, VRAM, temperatura, clocks e potência opcional percorrem domínio, IPC, GUI e CLI (`devices --type gpu`). O collector detecta driver/hardware ausente sem derrubar o agente e mantém campos sem suporte ausentes. Teste de collector e smoke agente→IPC→CLI com hardware NVIDIA real foram registrados em evidência anterior. Revisão local 2026-11-07 fez GUI e CLI preservar o estado não-`ok` quando há métricas de GPU válidas junto de falha parcial; integração Slint verifica a mensagem na página Performance e teste CLI verifica a política de impressão. A primeira tentativa de localizar o elemento GUI usou Overview e não é reprodução válida pré-correção. Fronteira interna fake do NVML verifica enumeração multi-GPU/IDs distintos, falha global, falhas parciais com preservação, zero dispositivos e opcionais ausentes. A capability matrix lista NVIDIA, AMD, Intel e Apple Silicon separadamente; só NVIDIA Linux pode aparecer `supported` quando NVML enumera hardware. AMD Linux tem adapter experimental por sysfs `amdgpu`, integrado ao snapshot, mas continua `unsupported` até homologação real. [ADR-014](adr/014-linux-intel-gpu-telemetry.md) define Level Zero Sysman opcional e rejeita DRM `fdinfo` para agregados device-wide; adapter Intel Linux experimental implementa descoberta e métricas best-effort, mas `gpu_intel` continua `unsupported` até homologação em hardware real. Apple Silicon e outros runtimes também seguem sem adapters. Fixtures não substituem homologação em hardware AMD/Intel real. |
| M4 — Saúde do disco, energia e bateria | Parcial | Primeira fatia Linux somente leitura de bateria em `/sys/class/power_supply`: estado, capacidade, energia, potência, temperatura e ciclos opcionais percorrem domínio, IPC, GUI e CLI (`devices --type battery`). Fixtures cobrem unidades, ausência e dados inválidos; smoke agente→IPC→CLI enumerou uma bateria real, mantendo temperatura não fornecida como ausente. SMART/NVMe, adapters macOS/Windows, alertas/histerese e validação em mais hardware seguem pendentes. |
| M5 — Prometheus, VictoriaMetrics e histórico avançado | Parcial | Pull OpenMetrics 1.0 em `crates/exporters`: `/metrics` loopback opt-in, snapshot existente, aliases sem dados privados, campos ausentes omitidos, allowlist tipada e métricas de saúde. Scrapes concorrentes (4), cabeçalho (4 KiB), timeout (2 s) e séries (128 dispositivos, 512 CPUs) limitados; CI `40318138924` passou em Linux/macOS/Windows para a versão anterior do exporter. Remote Write v1 codifica protobuf+Snappy e envia snapshots via cliente HTTPS sem redirects; opt-in, HTTP só em loopback, intervalos e fila validados, fila FIFO em memória bounded, backoff/jitter e Retry-After delta-seconds limitado; erros permanentes 4xx descartam lote. `/metrics` expõe enabled, fila (lotes/bytes), sucessos, retries, rejeições, descartes e timestamp do último sucesso; contadores usam OpenMetrics `counter` e os valores são voláteis. Gate local completo para `253cae9` passou (fmt, Clippy, testes workspace, build e laboratório/config); CI `40384814605` passou. A fatia de métricas de fila/status em `78afbe0` passou o gate local completo e CI `40385844421` passou; atualização documental `372f399` também passou no CI `40386053918`. Laboratórios isolados confirmaram `native_monitor_cpu_utilization_ratio` via API de séries VictoriaMetrics v1.154.0 e via API de consulta Prometheus v3.15.1 com `--web.enable-remote-write-receiver`; container/rede próprios removidos. O novo laboratório Prometheus em `2dc655a` passou CI `40389053378` em Linux/macOS/Windows, além de passar localmente. Vault auto-hospedado foi escolhido como destino de custódia, mas integração, identidade de acesso, rotação/revogação e recuperação ainda não foram definidas. O spool durável deve ficar no diretório de estado da aplicação por usuário; limites, retenção, caminho por OS, recuperação, limpeza e ativação seguem pendentes. A implementação atual mantém fila volátil. ADR-010 e ADR-011. |
| M6 — Gestão avançada de processos | Em andamento | Consulta da árvore somente de leitura (IPC minor 9, ADR-012) permanece separada do controle. A primeira fatia de controle foi aprovada em ADR-017: IPC tipado minor 11, PID + início, permissões normais do agente, revalidação, `pidfd` no Linux quando disponível, `SIGTERM`/`SIGKILL` em Unix e caminho nativo Windows que tenta `WM_CLOSE` antes da força. GUI pt-BR/en-US exige confirmação gentil, espera monotônica de 5 s, nunca força automaticamente e só então oferece segunda confirmação; eventos locais registram hora/PID/início/ação/resultado sem command line. Testes locais cobrem controlador com fixtures próprios, dispatch IPC para fixtures, round-trip tipado, prazo e interação dos botões. O gate local passou e a CI `42593553670` passou em Linux/macOS/Windows; os testes executam nos runners de cada OS, mas não substituem QA visual/manual da jornada. A identidade disponível tem granularidade de segundos, portanto não promete proteção atômica contra PID reuse onde a API não vincula a ação a handle estável; reavaliar esse risco após ensaios nativos. Helper, elevação, outras contas, prioridades e auditoria/retenção além do sink local seguem fora da primeira fatia e exigem decisões próprias. |
| M7 — Design, acessibilidade e estabilidade V1 | Em andamento | Semântica e navegação Slint, atalhos `Ctrl+1–5`, estados acessíveis de filtro/ordenação/processo e resumos de gráficos estão cobertos por testes unitários/integração; os catálogos pt-BR/en-US cobrem páginas e estados principais. As escolhas SI/IEC e confortável/compacta agora persistem em `ui-settings.toml`, com IEC/confortável como padrões (ADRs 015–016); “extra compacta” segue fora até haver limites visuais definidos. A paleta dark examinada passou AA de texto normal, com mínimo estático de 4,98:1; isso não verifica contraste não textual nem screenshot. Benchmark Slint software passou os budgets sintéticos registrados em M1. CI `42586623211` passou em Linux/macOS/Windows para `2504195`. Faltam revisão linguística integral, QA nativa por OS, leitores de tela reais, foco/HiDPI, estados/temas claros ou de alto contraste e soak/update/reboot/sleep. |
| M8 — Release público e manutenção | Em andamento | `CHANGELOG.md` tem resumo `[Unreleased]`; `CONTRIBUTING.md` registra toolchain, gates com `--locked`, auditoria RustSec/licenças e geração CycloneDX local. CI `42593553670` para `637ed3b` passou em Linux/macOS/Windows, incluindo instalação e execução de `cargo deny check licenses` na perna Linux; `cargo audit` também passou no gate. Auditoria corrigiu `pulldown-cmark` 0.13.5 → 0.13.6 e `async-executor` 1.14.0 → 1.14.1; auditoria atualizada não encontrou vulnerabilidades/yanked, mas reporta seis dependências transitivas não mantidas. MIT foi aprovado pelo usuário em 2026-11-09 e `LICENSE` usa `Copyright (c) 2026 Lucas Eufrasio`, conforme autorização para usar a autoria dos commits. O responsável aprovou todos os identificadores do inventário de terceiros em 2026-11-09; `deny.toml` e ADR-018 registram a política. SBOMs CycloneDX locais foram regenerados para os 13 pacotes do workspace, incluindo `native-monitor-process-control`; as referências `file://` continuam locais, e os arquivos não são release artifacts. Não há SLA/canal privado de disclosure adotado nem são pré-requisitos genéricos de open source; as instruções de reporte devem refletir os mecanismos realmente escolhidos pelo projeto. Checksums/proveniência e homologação de instalação/upgrade/desinstalação por OS continuam pendentes. O usuário escolheu artefatos macOS sem assinatura/notarização; validar/documentar a experiência do Gatekeeper. Outros tipos de assinatura permanecem conforme decisões específicas de distribuição, não como requisito universal para publicar o código open source. |

**Inventário inicial de licenças M8 (2026-11-09):** a união dos 13 SBOMs
CycloneDX 1.5 locais em `target/sbom-2026-11-09/`, com features padrão e todos
os alvos declarados, contém 656 componentes de terceiros (pares nome/versão),
35 expressões SPDX distintas e nenhum componente sem expressão. Entre elas, 14
componentes Slint declaram
`GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`;
`r-efi 6.0.0` declara `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. A presença de
alternativas no metadado não determina qual opção se aplica ou deve ser escolhida
para este produto. O responsável aprovou uma allow-list desses identificadores;
isso não é conclusão legal. Os artefatos e esta contagem
derivam do lockfile local e não são SBOMs de release. Em 2026-11-09, o
responsável aprovou todos os identificadores e alternativas desse inventário; a
política explícita está em [ADR-018](adr/018-third-party-license-allowlist.md).

**Gate de licenças M8 (2026-11-09):** `deny.toml` contém somente os 18
identificadores encontrados no inventário aprovado; `cargo deny check licenses`
(`cargo-deny` 0.20.2) passou localmente. A versão 0.20.2 é o release estável mais
recente conferido na data ([upstream EmbarkStudios](https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2),
[crates.io](https://crates.io/crates/cargo-deny/0.20.2));
seu Rust mínimo declarado é 1.88.0, compatível com o toolchain 1.98.1 do projeto.
A página oficial de advisories do cargo-deny não lista advisories publicados.
O workflow instala essa versão e executa o check na perna Linux. O workflow passou no
[actionlint 1.7.12](https://github.com/rhysd/actionlint/releases/tag/v1.7.12),
release estável mais recente conferido do upstream `rhysd/actionlint`; o binário
Linux x86_64 foi verificado contra o SHA-256 publicado
(`8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8`) e não
produziu diagnósticos. A página de [advisories](https://github.com/rhysd/actionlint/security/advisories)
não lista advisories publicados. Em 2026-11-09, as actions existentes foram
rechecadas e permaneceram nos releases estáveis atuais: [checkout v7.0.1](https://github.com/actions/checkout/releases)
(SHA fixado e commit assinado; nenhuma advisory publicada) e
[setup-rust-toolchain v2.0.1](https://github.com/actions-rust-lang/setup-rust-toolchain/releases)
(SHA fixado, commit sem assinatura; nenhuma advisory publicada). Fontes de
segurança: [checkout](https://github.com/actions/checkout/security/advisories),
[setup-rust-toolchain](https://github.com/actions-rust-lang/setup-rust-toolchain/security/advisories)
e [cargo-deny](https://github.com/EmbarkStudios/cargo-deny/security/advisories).

**Gate local M8 (2026-11-09):** em Rust 1.98.1, passaram:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace --locked` (**163 aprovados,
  0 falhas, 2 verificações manuais ignoradas**);
- `cargo build --workspace --locked`;
- `cargo audit` e `cargo deny check licenses`.

O cargo-audit atualizou o banco RustSec para 1.422 advisories e não encontrou
vulnerabilidades nem versões yanked; manteve seis avisos de dependências não
mantidas descritos acima. O workflow também passou em actionlint 1.7.12 local.
CI remota `42593553670` passou para o commit `637ed3b`; a perna Linux instalou
`cargo-deny 0.20.2` e aprovou o gate de licenças. A atualização documental em
`27bd5bb` também passou na CI `42595320776`.

**Auditoria de dependências M8 (2026-11-08):** `cargo audit` 0.22.2 atualizou
`rustsec/advisory-db` para o commit `7a40350e` (2026-11-08). A primeira varredura
encontrou `RUSTSEC-2026-0433` (CVSS 7.5) em `pulldown-cmark 0.13.5`, incluído por
Slint e também presente no parser de comentários de build do `prost-build`; a
resolução dry-run aceitou o patch 0.13.6, e `Cargo.lock` foi atualizado somente
nessa dependência. A primeira varredura posterior, com banco atualizado, não
encontrou vulnerabilidades, mas manteve seis avisos de crates não mantidos e um
aviso de versão yanked. `async-executor 1.14.0` tinha patch 1.14.1 compatível no
dry-run e foi atualizado no lock; uma nova varredura atualizada não encontrou
vulnerabilidades nem versões yanked, restando seis avisos de manutenção. O
`bincode` avisado não aparece no grafo ativo `cargo tree --target all -e all`;
`async-executor` está ativo no grafo Slint/AccessKit.
Os demais avisos são transitivos do stack Slint/winit/AT-SPI; não foram ignorados
nem removidos por supressão. `bincode` é dependência opcional de
`typed-index-collections` atrás da feature `bincode`, que não é ativada no grafo
de features do workspace; ainda assim o pacote permanece no `Cargo.lock` e é
apontado por `cargo audit`. `slint` e `slint-build` 1.18.1 eram as versões
estáveis mais recentes no crates.io em 2026-11-08, então não há patch estável
dos pais diretos para testar contra esses avisos. A verificação
`cargo deny check licenses` foi
executada sem configuração de projeto e falhou porque nenhuma licença, inclusive
MIT/Apache/BSD/Unicode/Zlib, está explicitamente allowlisted. É resultado
inconclusivo daquela execução, anterior à decisão e ao `deny.toml` documentados
abaixo; não é constatação de licença proibida.
Gate local após a atualização passou: fmt, Clippy, workspace tests (**149
aprovados, 0 falhas, 2 verificações manuais ignoradas**) e build `--locked`.
`cargo deny check advisories bans sources`, sem configuração específica,
terminou com `advisories FAILED`, `bans ok`, `sources ok`: cinco avisos ativos de
crates não mantidos (`pin-utils`, `ttf-parser`, `zbus_names`, `zvariant` e
`zvariant_derive`) são tratados como erro pelo deny; `bincode`, sexto aviso do
`cargo audit` no lockfile, não aparece no grafo `cargo tree --target all -e all`.
`bans`/`sources` emitiram avisos de versões transitivas duplicadas. Ainda faltam
política/relatório e scanner de licenças aprovados, tratamento sustentado dos
avisos transitivos, SBOM vinculado a artefatos de release e CI de supply chain.

**Rechecagem local RustSec (2026-11-09):** `cargo audit --no-fetch` verificou o
`Cargo.lock` atualizado usando a cópia local de 1.422 advisories; não encontrou
vulnerabilidades nem versões yanked. Repetiu seis avisos de manutenção
(`bincode`, `pin-utils`, `ttf-parser`, `zbus_names`, `zvariant` e
`zvariant_derive`). Como não houve `--fetch`, esta evidência não afirma que a base
esteja atualizada para a data da execução.

**Workflow de auditoria M8 (2026-11-08):** `.github/workflows/ci.yml` instala
`cargo-audit 0.22.2` e executa `cargo audit` somente na perna Linux, com
`permissions: contents: read` já definido. Clippy, testes e build também exigem
`--locked`, alinhados a `CONTRIBUTING.md`. A versão 0.22.2 é a versão estável
mais recente verificada no crates.io em 2026-11-08 (`rust-version = 1.88`, abaixo
do Rust 1.98.1 do projeto). As ações mantêm SHA completos e ambos os refs foram
resolvidos nas APIs de tags dos repositórios canônicos: `actions/checkout`
`v7.0.1` (`3d3c42e5…`) é o release estável mais recente, commit verificado como
assinado; `actions-rust-lang/setup-rust-toolchain` `v2.0.1`
(`a188e68d…`, publicado em 2026-11-02) é o release estável mais recente, commit
sem assinatura. A página oficial de advisories do checkout declara não haver
advisories publicados; para setup-rust-toolchain a rota do repositório retornou
404 e a busca pública não encontrou resultado, portanto não foi possível
confirmar ausência de advisories. Fontes consultadas em 2026-11-08: [checkout
releases](https://github.com/actions/checkout/releases), [ref e commit
checkout](https://api.github.com/repos/actions/checkout/git/ref/tags/v7.0.1),
[checkout advisories](https://github.com/actions/checkout/security/advisories),
[setup-rust-toolchain releases](https://github.com/actions-rust-lang/setup-rust-toolchain/releases),
[ref e commit setup-rust-toolchain](https://api.github.com/repos/actions-rust-lang/setup-rust-toolchain/git/ref/tags/v2.0.1),
[setup-rust-toolchain advisories](https://github.com/actions-rust-lang/setup-rust-toolchain/security/advisories),
[GitHub secure-use](https://docs.github.com/en/actions/reference/security/secure-use)
e [cargo-audit no crates.io](https://crates.io/crates/cargo-audit). `cache:
false` desativa a etapa interna de cache do setup action. O diff foi inspecionado
manualmente; `actionlint`, `yamllint`, `yq`, Ruby e PyYAML não estão disponíveis,
nenhum foi instalado, e não houve execução do GitHub Actions nesta fatia.
Portanto, CI de advisories está implementada no arquivo, mas sua execução remota
ainda não foi verificada.

**SBOM M8 local (2026-11-08):** `cargo-cyclonedx` 0.5.10, release estável mais
recente verificado no crates.io (Rust mínimo 1.86), gerou JSON CycloneDX 1.5 com
`--target all` e features padrão para os 12 pacotes do workspace. Foram
inspecionadas 12 saídas, somando 1.345 entradas de componentes (com dependências
repetidas entre SBOMs por pacote). Validação local em Python 3.13.5 confirmou
JSON parseável, formato/versão, referências únicas e fechamento das referências
de dependência; `sha256sum --check` passou para as 12 saídas. Artefatos e
manifesto de hashes estão somente em `target/sbom-2026-11-08/` (ignorado pelo
Git), pois os `bom-ref`/PURL de componentes do workspace incluem caminho local
`file://`. Esta checagem estrutural não é validação independente contra o schema
CycloneDX, auditoria de licenças nem evidência de conteúdo de um binário
empacotado; não publicar as saídas sem revisar refs, features e vínculo com o
build de release. O inventário registrou 35 expressões SPDX declaradas distintas
e nenhum componente sem expressão no BOM; expressões são dados brutos, não
decisão legal nem allow-list (incluem alternativas `LicenseRef-Slint-*`). O
instalador do gerador também reportou `xml-rs 0.8.19`
yanked em seu próprio lock de ferramenta; isso não alterou `Cargo.lock` do
produto. A geração foi adicionada a `CONTRIBUTING.md`; integração a release/CI,
SBOM por instalador final e checksums/proveniência continuam pendentes.

**Rechecagem do SBOM local (2026-11-08):** os 12 JSONs parsearam como CycloneDX
1.5; o inventário total continua em 1.345 componentes incluindo repetições por
pacote, os SBOMs do desktop incluem `pulldown-cmark 0.13.6` e
`async-executor 1.14.1`, e `sha256sum --check` passou para todos. Os arquivos
permanecem locais/ignorados e a ressalva de schema, licença e vínculo a release
acima permanece.

**Rechecagem do SBOM local (2026-11-09):** após adicionar
`native-monitor-process-control`, `cargo-cyclonedx 0.5.10` regenerou 13 SBOMs
CycloneDX 1.5 com `--target all`, agora em `target/sbom-2026-11-09/` (ignorado
pelo Git). Os 13 JSONs parsearam; a verificação estrutural local conferiu versão,
refs de dependência fechadas e presença do novo crate, totalizando 1.431
componentes de nível superior com repetições entre pacotes. `sha256sum --check`
passou para os 13 arquivos. Isso não é validação independente contra o schema
CycloneDX, revisão de licenças, SBOM de instalador nem artefato de release; refs
`file://` continuam específicas deste checkout.

**M6 — primeira fatia de controle de processos (2026-11-09):** implementada
conforme ADR-017, mantendo a consulta de árvore da ADR-012 somente de leitura.
O agente aceita apenas ação tipada e PID + início no IPC minor 11, revalida a
identidade e registra localmente timestamp/PID/início/ação/resultado via tracing,
sem command line. Linux usa pidfd quando disponível e revalida no fallback; Unix
usa SIGTERM/SIGKILL. O adapter Windows abre handle e valida creation time, envia
WM_CLOSE às janelas de topo correspondentes antes de encerrar forçadamente; este
host Linux não compilou nem executou esse caminho nativamente. A GUI localizada
em pt-BR/en-US exige confirmação gentil, aguarda cinco segundos monotônicos e só
oferece força com segunda confirmação, nunca automaticamente. Testes com
processos-fixture próprios cobrem ações gentil/forçada, identidade stale/ausente
e dispatch IPC; testes adicionais cobrem enumeração protobuf, prazo e interação
acessível dos botões. Gate local final: fmt, Clippy, build e workspace tests
(**163 aprovados, 0 falhas, 2 ensaios manuais ignorados**). Isso não é CI remota,
QA visual da aplicação em execução nem homologação Windows/macOS. O timestamp de
início continua com precisão de segundos e não constitui proteção atômica contra
PID reuse em APIs que não vinculam a ação a um handle estável. A primeira fatia
não inclui helper, elevação, outras contas, prioridades ou auditoria durável além
do sink local.

**Evidência visual Linux (2026-10-29):** inspeção do app conectado em workspace privado KWin, Linux x86_64, nos tamanhos 960 × 640 e 1200 × 760. Foram corrigidos título/status truncados, tamanho inicial fixo que impedia redimensionamento real e cartões da visão geral que não cabiam em largura reduzida. A tela de processos e métricas reais também foi observada nos dois tamanhos; capturas ficaram apenas no armazenamento temporário de QA porque incluem nomes/PIDs do host. Isto não homologa Wayland/X11, outros sistemas, HiDPI, temas, leitor de tela ou toda a jornada de entrada: o backend Wayland do `winit` encerrou com panic ao receber dados de ponteiro e a interação de filtro não foi comprovada naquela sessão. A interação foi exercitada depois em uma VM isolada, descrita abaixo; isso não substitui a validação nativa. O teste headless existente cobre seleção, navegação, densidade e idioma, mas não substitui essa validação visual/interativa.

#### Fatia M2 — continuidade das taxas de volumes (2026-10-29)

Lacuna tratada: o relógio global de I/O permitia calcular taxas para um volume
recém-enumerado, sem duas observações desse volume. O coletor comum agora usa
counters cumulativos e tempo monotônico por entrada montada, conforme ADR-005.
Não altera schema IPC, capacidades nem a distinção volume/dispositivo físico;
as taxas opcionais seguem pelo caminho existente agente → IPC → GUI/CLI.

| Critério observável | Implementação e evidência automatizada local |
|---|---|
| Volume novo/reaparecido não apresenta taxa até a segunda observação contínua; remoção elimina entrada e baseline. | `VolumeIoTracker`, teste `hotplug_preserves_continuous_rates_and_warms_new_volumes`: fixture com adição, remoção, reaparecimento e enumeração vazia. |
| Volumes contínuos usam deltas cumulativos em bytes/s e preservam capacidade; inatividade válida pode ser zero. | Mesmo teste: 1.000 B em 2 s = 500 B/s, capacidade preservada e delta zero explícito. |
| Troca observável de dispositivo/filesystem no mount invalida a baseline; capacidade desconhecida continua ausente. | `replacement_device_or_filesystem_at_same_mount_starts_a_new_baseline`. |
| Reset de leitura/escrita e intervalo nulo/invertido mantêm a taxa afetada ausente; amostra atual permite nova baseline. | `counter_resets_and_invalid_intervals_remain_absent_then_recover`: resets independentes e recuperação da leitura. |

**Verificado:** `cargo test -p native-monitor-collectors-common` passou, 6 testes
(3 novos de continuidade, substituindo o teste escalar de taxas de volume).
`cargo fmt --all -- --check` passou. Como verificação complementar, passaram:

```sh
cargo clippy --workspace --exclude native-monitor-desktop --all-targets -- -D warnings
SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace --exclude native-monitor-desktop
cargo build --workspace --exclude native-monitor-desktop
```

A suíte sem GUI teve 68 testes aprovados e 1 ignorado (benchmark manual de
processos, não aplicável à mudança). Isso inclui o round-trip IPC existente que
preserva taxa de volume ausente; não é E2E de hotplug.

**Bloqueio inicial do gate completo:** os comandos documentados
`cargo clippy --workspace --all-targets -- -D warnings` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` falharam no build de
`yeslogic-fontconfig-sys`: `pkg-config` não estava instalado. Os metadados de
desenvolvimento do Fontconfig também não foram encontrados. Naquela tentativa, o
build completo não foi executado após esse bloqueio; somente o build sem GUI
acima passou. A instalação de `pkg-config` e `libfontconfig1-dev` foi
autorizada, mas `sudo -n apt-get update` recusou por exigir senha; nenhum pacote
foi instalado. O `cargo` do PATH inicial era um shim Mise sem versão; os checks
foram executados com `$HOME/.cargo/bin` à frente do PATH, usando Rustup e o
toolchain 1.98.1 do repositório, sem reconfigurar defaults globais.

**Gate completo verificado após instalação pelo responsável:**
`pkg-config --modversion fontconfig` confirmou Fontconfig 2.15.0. A primeira
retomada excedeu o limite de 120 s durante a compilação dos testes; a repetição
com limite de 600 s concluiu com sucesso os quatro comandos integrais de
`CONTRIBUTING.md`, usando Cargo 1.98.1:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace
cargo build --workspace
```

Resultado local Linux x86_64: **93 testes aprovados, zero falhas e 1 ignorado**
(benchmark manual de processos, não aplicável à fatia), incluindo os 25 testes
desktop com backend de teste Slint. Build completo passou. Essa evidência remove
o bloqueio do gate local, mas não é CI novo, QA visual ou homologação nativa.

**Smoke real Linux de montagem/remoção/remontagem:** duas execuções independentes
de um ensaio temporário não versionado, cada uma com dois ciclos.
`unshare --user --map-root-user --mount` isolou os mounts; um tmpfs privado
isolou o IPC e a configuração do agente.
Um bind mount de diretório existente foi criado/removido dentro do namespace,
sem montar dispositivos nem alterar mounts do host. Os binários reais
`target/debug/native-monitor-agent` e `target/debug/native-monitorctl` comprovaram:
agente pronto, enumeração do mount, ambas as taxas `—` na primeira amostra,
taxas presentes após continuidade, desaparecimento após desmontagem e novo
aquecimento na remontagem. As duas execuções terminaram com `PASS: production
collector -> agent -> IPC -> CLI mount/remove/remount journey`; agente próprio
encerrado via SIGINT e mounts/IPC descartados ao sair. Exportação permaneceu
desativada por default. O ensaio só imprimiu resultados de asserções, sem nomes
ou métricas privadas. Isto verifica a reenumeração de mounts pelo caminho real,
não conexão USB, hotplug físico, aferição de throughput ou GUI.

**QA interativo Linux em VM privada (2026-11-07):** Ubuntu Server 24.04, QEMU/KVM,
root-port PCIe e disco virtio descartável; arquivos da VM, logs e capturas
permaneceram em armazenamento isolado fora do repositório. Xvfb
privado 1440 × 900, Openbox, Slint software, locale en-US; janela de 1120 × 720.
O binário desktop corrigido foi compilado deste checkout (`9c2e7fb` mais a
alteração não commitada descrita a seguir), iniciado junto ao agente; `monitorctl
status` reportou `running` e as capturas foram inspecionadas.

- Storage refletiu a presença do disco virtual descartável. Uma gravação
  sintética limitada foi observada de forma concordante na CLI e na GUI; após a
  carga, ambas voltaram a indicar inatividade.
- A remoção do ponto de montagem e o hot-unplug virtual foram refletidos pela CLI
  e pela GUI, sem mount stale. Após reconectar e montar novamente, as taxas
  aqueceram antes de voltar ao estado de inatividade.
- Processo sintético `nsm-filter-v2`: a tela mostrou 1 resultado; busca sem
  correspondência mostrou 0 e estado vazio localizado; limpar o filtro restaurou
  180 processos. O processo de fixture foi encerrado e verificado ausente.
- Capturas representativas inspecionadas: `fixed-storage-focused.png`,
  `storage-write-24.png`, `storage-write-idle.png`,
  `fixed-storage-unmounted.png`, `fixed-storage-replugged.png`,
  `fixed-filter-match.png`, `fixed-filter-empty.png`,
  `fixed-filter-cleared.png` e `fixed-network-view.png`; todas permanecem fora
  do repo.

**QA visual/interativa Linux em VM QEMU privada (2026-11-08):** QMP confirmou a
VM ativa. A conta do guest foi recuperada do cloud-init da própria VM; a chave de
QA local foi conferida contra a chave pública autorizada, e o fingerprint ED25519
confirmado pelo usuário foi revalidado e fixado apenas no `known_hosts` privado
desta execução. Guest Ubuntu Server 24.04 x86_64, `Xvfb :99` em 1440 × 900,
Openbox e renderer Slint `winit-software`; o agente já estava ativo e
`monitorctl status` reportou `running`, protocolo IPC 1.10. A GUI do checkout
(`dcd3b98` mais as alterações locais desta QA) foi compilada e copiada ao guest;
SHA-256 do binário observado: `2b00431d2b2b84d79575b53b20d19cc3b748635c097a579ea82b190198568724`.
Preferências foram isoladas por `XDG_CONFIG_HOME` privado.

- A primeira tela en-US mostrou agente ativo, métricas, amostras dos gráficos e
  layout sem cortes aparentes. Alternar para pt-BR atualizou título, páginas,
  estados, decimais com vírgula e unidades. Screenshots de janela real foram
  inspecionados em 1120 × 720 e após resize em 960 × 640 e 1200 × 760.
- Em Processes, filtrar `Xvfb` mostrou 1 processo; uma busca sem correspondência
  mostrou 0 e a mensagem vazia localizada; limpar o filtro restaurou a lista
  (185 processos após, contra 186 antes, devido à variação natural de processos
  do guest). Os atalhos Ctrl+2/Ctrl+4 abriram Processes/Storage.
- Em Storage, a configuração isolada nova iniciou em IEC. Selecionar SI mudou os
  valores exibidos para MB/GB e gravou `byte_unit_system = "si"`; após encerrar
  somente o processo QA e reabrir o binário, SI permaneceu selecionado. Em
  960 × 640 a lista de dispositivos continuou navegável por scroll.
- O renderer alternativo `SLINT_BACKEND=winit-femtovg` iniciou em Xvfb com
  `LIBGL_ALWAYS_SOFTWARE=1`; o processo carregou Mesa GLX/LLVM e produziu a
  captura inspecionada `femtovg-overview.png`. Isto valida startup/renderização
  virtual femtovg sobre GL software, não aceleração ou homologação de GPU física.
- Para a checagem nativa de acessibilidade, `org.a11y.Status.IsEnabled` foi
  ativado apenas durante a sessão e restaurado ao valor inicial `false`. O
  processo apareceu no bus AT-SPI; a janela `System Monitor` expôs sua árvore
  AccessKit (10 filhos) e cache com `Main navigation`, `Processes` e `Total CPU`.
  Após alternar idioma, o cache expôs `Navegação principal`, `Processos` e
  `CPU total`. Isto confirma a ponte Linux AT-SPI neste ambiente virtual; não
  houve validação de fala/interação com Orca ou outro leitor de tela.
- A revisão visual encontrou o contador de volumes colado ao rótulo “Units”/
  “Unidades”. A linha de cabeçalho Storage agora define `spacing: 8px`; o teste
  `clicking_a_process_row_emits_its_pid_and_start_time_identity` verifica a
  distância geométrica mínima. Remover o espaçamento fez o teste falhar; com a
  correção ele passou. Capturas posteriores confirmaram o espaçamento em en-US
  nos três tamanhos e em pt-BR em 1120 × 720 e 1200 × 760.
- Evidências visuais mantidas fora do repositório e inspecionadas: overview,
  `current-pt-br.png`, `current-filter-match.png`, `current-filter-empty.png`,
  `current-filter-cleared.png`, `storage-spacing-1120x720.png`,
  `storage-spacing-960x640.png`, `storage-spacing-960x640-scrolled.png`,
  `storage-spacing-1200x760.png`, `storage-spacing-pt-br-1120x720-settled.png`
  e `femtovg-overview.png`.
  A regressão geométrica passou após a correção; gate local final: fmt, Clippy,
  `SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
  1 teste manual ignorado**) e `cargo build --workspace`. Nenhum fixture
  adicional foi deixado rodando por esta jornada.

Isto é QA visual/interativa Linux **virtual** com renderer software, não
homologação de GPU, Wayland, HiDPI físico ou instalação nativa. Screenshots por
OS, validação nativa multiplataforma e critérios que dependem de hardware real
continuam pendentes. O binário foi gerado de alterações ainda não commitadas;
nenhum commit ou push foi feito.

**Checagens complementares M2 (2026-11-07):** no guest, rodei o script existente
`scripts/validate-linux-block-io.sh` com `--size-mib 8` no filesystem ext4 do disco
virtio hotplugado. `lsblk` identificou somente um dispositivo inteiro de bloco,
sem partição filha; `devices --type block` mostrou uma única entrada. O snapshot do agente e
os counters sysfs independentes indicaram tendência/unidade compatível em uma
amostra limitada; a janela não é alinhada com precisão, portanto não constitui
benchmark nem taxa exata. O script removeu o arquivo descartável. Após hot-unplug,
a CLI deixou de listar o dispositivo após a atualização do snapshot; no replug,
ele reapareceu aquecendo e retornou à inatividade após a segunda amostra.

A página Network e `devices --type network` exibiram loopback e adaptador
virtual como interfaces distintas; a classificação conservadora não os trata como
contagem de hardware físico e não infere tipo físico/virtual por nome ou driver.

**Permissões M2 — evidência parcial:** agente e CLI consultaram `/proc/diskstats`
e counters sysfs como usuário sem privilégios elevados; os arquivos consultados
eram legíveis sem `sudo` no caminho do coletor. Pelo código,
falha ao ler `/proc/diskstats` resulta em `MetricStatus::Error`; falha ao ler a
topologia sysfs exclui a entrada afetada. Nenhuma negação real foi simulada, e a
exclusão por sysfs não traz motivo por dispositivo na CLI. A matriz de negação por
OS permanece pendente, sem tentativa de elevar privilégios.

**Classificação local de interfaces (2026-11-07):** domínio e coleta agora
transportam `NetworkInterfaceKind`; o adaptador Linux marca `Loopback` somente
quando `/sys/class/net/<nome>/flags` contém `IFF_LOOPBACK`. Flags ausentes,
inválidas, enum desconhecido, protocolos anteriores ao minor 10 e todas as
interfaces sem evidência positiva resultam em `Unknown`. Não são inferidos
`Physical`, `Virtual`, VPN/túnel, bridge nem deduplicação. O campo protobuf
aditivo usa valor zero `Unknown`; a GUI mostra coluna `Tipo` localizada e linha
acessível; `monitorctl devices --type network` mostra `kind` (`loopback` ou
`unknown`). Testes cobrem flags sysfs válidas/ausentes/inválidas, mapeamento do
snapshot para protobuf, round-trip do frame, fallback de versão/enum, labels e
renderização acessível da linha.

**Smoke integrado Linux após a classificação:** no guest Ubuntu 24.04, o agente
respondeu `IPC protocol: 1.10`; a CLI distinguiu loopback de adaptador
virtual, coincidindo com a classificação derivada das flags nativas. A integração
Slint encontrou as linhas acessíveis com os rótulos `Loopback` e `Desconhecido`; o
desktop atualizado permaneceu ativo sem panic. Binários da VM foram comparados
por SHA-256 aos builds locais. Esta VM Linux x86_64 virtualizada não valida NIC
física ou outro sistema operacional.

**Fatia de auditoria M3 GPU (2026-11-07):** a GUI agora mantém o status de coleta
GPU não-`ok` visível ao lado de dispositivos válidos tanto na GUI quanto em
`monitorctl devices --type gpu`, em vez de apagá-lo quando a lista não está vazia;
`ok` continua sem mensagem redundante e o status da GUI é exposto como label
acessível. O teste CLI cobre estado degradado com dispositivo válido e mantém a
saída para lista vazia. A primeira tentativa de localizar o elemento apontou para
Overview e não é evidência válida de reprodução pré-correção. O teste foi
corrigido para a página Performance e agora verifica o estado visível; não se
afirma uma reprodução válida antes da mudança visual. A coleta NVML foi
separada internamente em `NvidiaDeviceSource` e `collect_devices`, sem mudar a API
pública nem o carregamento dinâmico. Fakes determinísticos confirmam 2 IDs
distintos, preservação de uma GPU válida com outra falhando, `Error` para falha de
enumeração, falha de todos os dispositivos como `TemporarilyUnavailable`,
`Unsupported` para zero GPUs e ausência (não zero inventado) de métricas opcionais.
O registro de capabilities lista `gpu_nvidia`, `gpu_amd`, `gpu_intel` e
`gpu_apple_silicon`; famílias sem adapter dizem explicitamente `unsupported` /
`no native adapter`. NVIDIA acompanha a disponibilidade NVML; AMD e Intel têm
adapters Linux experimentais, mas permanecem `unsupported` até homologação real.
Teste verifica esses estados e fontes. Teste IPC confirma preservação de
nome/status/fonte de cada backend no handler e round-trip protobuf por frame; a
CLI imprime essa lista sem filtrar famílias não homologadas. Isso melhora
descoberta e verifica lógica/semântica local, mas não equivale a falha induzida em
hardware/driver real nem homologa outro modelo/fabricante.

**Adapter experimental AMD Linux (2026-11-07):** ADR-013 define leitura somente
de `/sys/class/drm/cardN/device` quando PCI vendor é `0x1002` e o link de driver
é `amdgpu`. A identidade usa PCI BDF; os campos consultados são `gpu_busy_percent`
(%), `mem_info_vram_used/total` (bytes), hwmon `temp1_input` (millicelsius) e
`freq1_input/freq2_input` (Hz). Produto é opcional, cada métrica inválida ou
ausente fica ausente, e o adapter não escreve sysfs. Potência permanece ausente:
`power1_average` não distingue com segurança a GPU do SoC/CPU em APUs. Duas GPUs
AMD são integradas à lista de métricas quando descobertas, sem mudar
`gpu_amd=unsupported`; a capability registra que falta validação de hardware.
Fixtures verificam vendor/driver, descoberta multi-GPU/BDF, campos opcionais,
valor fora de faixa, conversões e ausência de potência mesmo com fixture
`power1_average`. O VRAM reportado em APU pode ser carveout UMA; GTT/memória
compartilhada não é incluída. Não houve homologação em hardware/firmware AMD real; portanto, os fixtures
não alteram o estado `unsupported` da capability.

**Adapter experimental Intel Linux (2026-11-07):** ADR-014 seleciona Level Zero
Sysman opcional; o adapter agora carrega `libze_loader.so.1` dinamicamente (sem
link obrigatório), filtra dispositivos GPU Intel por vendor/type e usa UUID
preferencialmente, com BDF como fallback. Fakes cobrem descoberta com múltiplos
dispositivos, filtragem de outro vendor/NPU, falha parcial, métricas/counters e
runtime ausente. Utilização usa engine `ALL` em snapshots sucessivos; VRAM só é
calculada de um único módulo raiz `DEVICE` usando `physicalSize` e `free`; sensor
GPU e `actual` do domínio gráfico são best-effort. Clock de memória não é exposto;
potência só é candidata pela extensão explícita de domínio GPU. A capability
continua `unsupported` até homologação real em hardware Intel. A especificação
confirma engine `ALL` com atividade calculada por
deltas de `activeTime/timestamp`, memória `DEVICE` separada de `SYSTEM`, clocks
GPU em MHz e sensores de temperatura em °C. Potência só é candidata quando o
domínio é explicitamente `GPU`; contadores de energia são µJ com timestamp µs.
Clock de memória requer validação de unidade no runtime/driver, e `size` do
Sysman memory state é deprecated desde v1.15. DRM `fdinfo` i915/xe é por cliente
(engine em ns e memória por região), portanto não se agrega como utilização ou
VRAM total sem risco de omissões/dupla contagem. Capability Intel segue
`unsupported`/`hardware validation pending` até homologação real.

**Gate local do adapter Intel (2026-11-07):** com Rust 1.98.1 documentado pelo
projeto, passaram `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (125 aprovados, 0 falhas, 1
benchmark manual ignorado) e `cargo build --workspace`; `git diff --check` também
passou. Um probe C compilado contra os headers oficiais Level Zero 1.20.41
comparou tamanhos e offsets-chave ABI com o teste Rust, e os fakes exercitaram
descoberta e semântica sem GPU. Isto não verifica o loader instalado nem substitui
execução em runtime/hardware: o ambiente de validação não disponibilizou runtime
nem hardware Intel para ensaio. Nenhuma CI nova foi executada; `gpu_intel`
continua `unsupported`.

**Auditoria M0–M8 e decisões atualizadas (2026-11-09):** a tabela acima continua sendo
o status de aceite; nenhum marco foi promovido a concluído. Há trabalho local
testável em M0/M1/M7 (renderer/acessibilidade no ambiente Linux, orçamento de
interação ainda sem limite acordado e cobertura de localização), mas screenshots
e comportamento nativo de macOS/Windows/Linux exigem os respectivos ambientes.
M2–M4 ainda combinam adapters/fixtures implementáveis com ensaios de hardware,
permissão ou OS externo; M3 não pode habilitar Intel/AMD sem homologação real.
M5 tem Vault auto-hospedado e diretório de estado por usuário para o spool
escolhidos, mas exige definir integração/ciclo de vida de credenciais e limites,
retenção e recuperação do spool; M6 já tem
decisão para a primeira fatia sem privilégio adicional, mas ainda exige ensaios
nativos por OS e decisão própria caso helper, outras contas ou prioridades sejam
retomados. M7 também depende de soak,
update/reboot/sleep e ensaios de acessibilidade/renderer; M8 tem titular/ano,
aviso MIT e `LICENSE` definidos, mas aguarda revisão/allow-list de licenças de
terceiros e validação de instaladores por OS. Não se estabeleceu SLA
nem canal privado de disclosure; estes não são tratados como pré-requisitos
genéricos de open source. Assinatura/notarização continuam vinculadas aos formatos
de distribuição específicos registrados abaixo; para macOS, a decisão atual é não
assinar nem notarizar os artefatos. A fatia M7 corrige “Swap” para “Memória swap”
em pt-BR nos dois layouts responsivos e verifica ambos os idiomas pelo seletor
no teste de interação Slint; `MetricPanel` expõe o próprio título como rótulo
acessível. Isto fecha apenas essa string, não o catálogo geral nem M7.

**M0 — gráfico com 1.200 amostras (2026-11-07):** o teste local
`five_minute_chart_history_handles_1200_samples_with_bounded_peak_preserving_output`
passa por `select_history` com 1.200 amostras determinísticas a cada 250 ms,
confere janela de cinco minutos, saída limitada a 72 pontos e preservação de picos
em buckets distintos. Isso verifica preparo/downsampling da série, não renderização
Slint, responsividade, ausência de freeze ou o target provisório de p95 da seção 13;
renderer alternativo e validação de acessibilidade por OS continuam pendentes.
M0 permanece parcial. Gate local em Rust 1.98.1 passou: fmt, Clippy, build,
`git diff --check` e `SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace`
(**141 aprovados, 0 falhas, 1 teste manual ignorado**); nenhuma CI nova foi
executada.

**Regressão de localização M7 (2026-11-07):** no estado de memória disponível,
o cartão combinava uma descrição portuguesa (`utilizado`) com status en-US.
`memory_status_description` agora aplica a localização também quando há valores
válidos; teste de regressão verifica a frase completa em pt-BR e en-US e preserva
os estados sem valor. O catálogo geral de strings e os demais critérios de M7
continuam pendentes.

**Separadores decimais M7 (2026-11-07):** `Locale::format_decimal` centraliza
vírgula em pt-BR e ponto em en-US nos formatadores numéricos da GUI: CPU/núcleos,
GPU e potência, bateria, processos, memória/VRAM, taxas de bytes, IOPS, latência,
ocupação e resumos acessíveis dos gráficos. Unidades, precisão e arredondamento
foram preservados; ausência continua `—`. Testes cobrem as duas localidades nos
formatadores e na linha de processo.

**Agrupamento de milhares M7 (2026-11-07):** `Locale::format_integer` agrupa
milhares com ponto em pt-BR e vírgula em en-US; `format_decimal` aplica o mesmo
agrupamento somente à parte inteira, sem alterar precisão nem arredondamento.
Contagens de processos/amostras, idades da amostra/processo e temperatura/clocks
de GPU usam os formatadores localizados. Testes verificam valores grandes nas
duas localidades, negativos, arredondamento, precisão e ausência (`—`); PID, PPID,
de strings gerais continua pendente; a fatia de catálogos para contadores foi
registrada abaixo.

**Unidades SI/IEC M7 (2026-11-07):** a página Armazenamento agora permite escolher
SI (base 1000) ou IEC (base 1024), com IEC mantido como padrão compatível. A
preferência persiste em `ui-settings.toml` no diretório de configuração do usuário
de cada OS; ausência usa o padrão, configuração inválida é ignorada com aviso e
falha de gravação não impede o uso durante a sessão. A opção formata capacidade,
taxas, RAM/swap, VRAM e memória por processo sem alterar IPC/coleta. Testes cobrem
conversões nas duas localidades, rótulos e unidades, seleção/radio acessível,
persistência por leitura após gravação, fallback, validação e caminhos de
configuração Linux/macOS/Windows. O mapeamento de diretórios dos três OS é teste
unitário; runtime/build nativo macOS/Windows continua dependendo de CI/hosts desses
sistemas. Gate local em Rust 1.98.1 passou: fmt, Clippy, build, `git diff --check`
e `SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**134 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada. Decisão registrada em
[ADR-015](adr/015-ui-byte-unit-preference.md).

**Catálogos de contadores M7 (2026-11-07):** mensagens de contagem de processos,
volumes e interfaces de rede foram extraídas para `apps/desktop/resources/i18n/`
em TOML pt-BR/en-US e ligadas às propriedades da GUI. A seleção singular/plural
e a formatação numérica continuam em código, mantendo os recursos apenas como
texto. Testes carregam/validam ambos os catálogos e verificam zero, singular,
plural e 1.234/1,234 em cada tipo de contador; detalhes da contagem de processos
na visão geral também usam os recursos. O restante do catálogo geral da GUI
permanece pendente. O ensaio de interação Slint também confirmou que os textos
pt-BR chegam aos rótulos acessíveis das páginas Processos, Armazenamento e Rede.
Gate local em Rust 1.98.1 passou: fmt, Clippy, build, `git diff --check` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**135 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada.

**Inventário e Visão geral i18n M7 (2026-11-07):** a varredura das expressões
inline restantes agrupou textos por cabeçalho, densidade, navegação e páginas
Visão geral, Processos, Desempenho, Armazenamento e Rede. Nesta fatia, o indicador
de atualização contínua, os cinco títulos dos painéis de métricas e os dois
títulos dos gráficos da Visão geral foram movidos aos catálogos TOML pt-BR/en-US.
Testes de catálogo cobrem os oito rótulos nas duas localidades; a integração
Slint verifica o indicador e os cinco painéis nas duas línguas após alternar a
localidade. As outras strings das páginas continuam pendentes. Gate local em
Rust 1.98.1 passou: fmt, Clippy, build, `git diff --check` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**139 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada.

**Lista e filtro de Processos i18n M7 (2026-11-07):** inventário da página
identificou a seção da lista, o campo de filtro (placeholder e nome acessível),
o cabeçalho ordenável de nome e os estados sem resultados/aguardando conexão.
Esses seis textos foram externalizados para TOML pt-BR/en-US; estados do painel
de seleção/árvore e o cabeçalho de memória com unidade dinâmica continuam fora
desta fatia. Testes verificam cada recurso nas duas localidades e a integração
Slint confere rótulos, placeholder e ambos os estados vazios após trocar o idioma.
Gate local em Rust 1.98.1 passou: fmt, Clippy, build, `git diff --check` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**140 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada.

**Inventário e cabeçalho i18n M7 (2026-11-07):** inspeção das expressões de
idioma em `main.slint` e dos ramos de `Locale` em Rust agrupou as strings ainda
embutidas em cabeçalho/estado do agente, navegação/densidade, visão geral,
processos, desempenho, armazenamento e rede. Nesta fatia, título, descrição,
estado ativo/indisponível do agente e ações de configurar início/tentar conexão
foram movidos para os catálogos TOML pt-BR/en-US e ligados também ao título da
janela. Teste unitário verifica os seis recursos nas duas localidades; a
integração Slint alterna pt-BR/en-US e confirma título e ação acessíveis em cada
idioma. Rótulos dentro das páginas, densidade e demais estados dinâmicos continuam
embutidos naquele ponto e ficaram para fatias próprias.

**Navegação lateral i18n M7 (2026-11-07):** seção, nome acessível do tab-list,
cinco destinos, aviso de coleta em segundo plano e rótulos visível/acessível dos
atalhos `Ctrl+1–5` foram movidos para TOML pt-BR/en-US. Teste unitário valida os
recursos nas duas localidades; a integração Slint confere os cinco tabs, papel e
nome da navegação e rótulo acessível do atalho após alternar os idiomas. Densidade
e os títulos/estados dentro das páginas ficaram para a fatia seguinte. Gate local em Rust
1.98.1 passou: fmt, Clippy, build, `git diff --check` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**137 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada.

**Densidade e títulos de páginas i18n M7 (2026-11-07):** rótulos visíveis e
acessíveis dos controles confortável/compacta foram movidos aos catálogos TOML
pt-BR/en-US. Os cabeçalhos das cinco páginas usam os mesmos recursos localizados
dos destinos correspondentes da navegação, sem duplicar traduções. Testes de
catálogo verificam as quatro formas de densidade nas duas localidades; a
integração Slint troca o idioma, confere os rótulos acessíveis dos controles e
verifica o título ativo de Overview, Processes, Performance, Storage e Network em
pt-BR/en-US. Outras strings e estados internos das páginas continuam pendentes.
Gate local em Rust 1.98.1 passou: fmt, Clippy, build, `git diff --check` e
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**138 aprovados, 0 falhas,
1 teste manual ignorado**); nenhuma CI nova foi executada.

**Página Desempenho i18n M7 (2026-11-07):** sete textos estáticos — intervalos
de 1/5 minutos, títulos dos gráficos de CPU/memória, título e estado sem dados
por núcleo e título da seção GPU/VRAM — foram externalizados para TOML pt-BR e
en-US. O teste de catálogo verifica todos os valores nos dois idiomas; a
integração Slint alterna a localidade e confirma os rótulos acessíveis dos
intervalos, o estado vazio de CPU por núcleo e as seções de CPU e GPU/VRAM. Não
cobre renderização visual ou acessibilidade nativa por OS; naquele ponto, o texto
compartilhado “agora” dos gráficos, status dinâmicos e outras strings das páginas
seguiam pendentes. Gate local em Rust 1.98.1: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**142 aprovados, 0 falhas,
1 teste manual ignorado**), `cargo build --workspace` e `git diff --check`
passaram. Nenhuma CI nova foi executada; M7 permanece em andamento.

**Catálogos M7 de gráficos, Storage e Network (2026-11-07):** os textos
“agora/now” e “Aguardando amostras/Waiting for samples” foram externalizados
para a seção `charts` dos catálogos TOML pt-BR/en-US. Também foram movidos para
TOML os cabeçalhos de volumes e atividade de bloco, os rótulos SI/IEC e os
cabeçalhos de interfaces de rede; as labels acessíveis usam os mesmos recursos.
Os catálogos são aplicados no início da GUI e na troca de idioma. Testes Slint
verificam os textos acessíveis nos gráficos de Visão geral/Desempenho e nas
páginas Storage/Network em ambas as línguas, incluindo os rótulos acessíveis dos
seletores SI/IEC; testes de catálogo validam os valores dos dois idiomas. Nenhuma
coleta ou contrato IPC foi alterado. Gate local com Rust 1.98.1 passou:
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**144 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Nenhuma CI nova foi
executada; renderização visual e acessibilidade nativa por OS continuam pendentes.

**Detalhes e estados da árvore de processos i18n M7 (2026-11-07):** rótulos
“Início/Started”, “RAM/Memory”, instrução sem seleção e título somente leitura da
árvore estão nos catálogos pt-BR/en-US e ligados às labels acessíveis. Loading,
truncamento, árvore vazia, processo ausente e erro agora são estados tipados,
traduzidos pelo catálogo; o estado atual é reapresentado no idioma novo ao trocar
a localidade, sem aguardar uma nova consulta para traduzir texto já visível. A
integração Slint verifica detalhes e árvore truncada visíveis em inglês, troca
para pt-BR enquanto a árvore continua visível e confirma os textos atualizados; o
teste de catálogo cobre todos os estados e fallback de código desconhecido.
Gate local com Rust 1.98.1 passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**145 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Nenhuma CI nova foi
executada; M7 e QA visual/nativa por OS permanecem em andamento.

**Faixas temporais dos gráficos i18n M7 (2026-11-07):** “início/start”,
“histórico/history”, “sem dados/no data” e o padrão de minutos atrás foram
movidos para os catálogos pt-BR/en-US. A seleção do intervalo agora retorna um
estado tipado, não uma string em português que depois precisa ser interpretada;
o template de minutos valida exatamente um token `{minutes}` e usa agrupamento
numérico conforme a localidade. O último estado das duas séries fica disponível
ao callback de idioma, que reaplica os rótulos imediatamente sem esperar outra
consulta IPC. O teste Slint alterna pt-BR → en-US → pt-BR e confere os dois
rótulos acessíveis dos gráficos; testes de catálogo cobrem início, histórico,
sem dados, minutos e agrupamento `1.234`/`1,234`. Gate local em Rust 1.98.1:
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**144 aprovados, 0 falhas,
1 teste manual ignorado**), `cargo build --workspace` e `git diff --check`
passaram. Nenhuma CI nova foi executada; renderização visual e QA nativo seguem
pendentes, e M7 permanece em andamento.

**Estados de disponibilidade de métricas i18n M7 (2026-11-07):** os estados
`ok`, `warming_up`, `unsupported`, `temporarily_unavailable` e `error` agora
usam chaves dos catálogos pt-BR/en-US, em vez de literais na função de
apresentação em `main.rs`. A mesma resolução é usada nos estados de GPU, bateria,
CPU, memória e I/O de bloco; códigos desconhecidos continuam visíveis sem serem
alterados. O teste Slint injeta o estado `warming_up` no detalhe acessível do
cartão de CPU e, ao alternar pt-BR → en-US → pt-BR, verifica “Aquecendo” →
“Warming up” → “Aquecendo”. Testes de catálogo verificam os cinco estados nos dois
idiomas e preservação de estado desconhecido. Gate local com Rust 1.98.1 passou:
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**145 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Nenhuma CI nova foi
executada; M7 continua em andamento e QA visual/nativa por OS não foi feita.

**Estados de capacidade e listas vazias i18n M7 (2026-11-07):** mensagens de
protocolo antigo sem suporte a GPU, bateria, volumes, taxas de I/O por volume,
interfaces de rede e I/O de bloco foram externalizadas via template e descritores
tipados em `[capability-status]`; também estão no catálogo os estados sem volumes
montados e sem interfaces de rede. O Slint expõe esses estados como labels
acessíveis. A integração alterna pt-BR/en-US nos cartões de bateria/GPU e nas
páginas Storage/Network, verificando as mensagens visíveis em ambos os idiomas;
testes de catálogo cobrem todas as capacidades e listas vazias. Gate local em
Rust 1.98.1 passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**146 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Nenhuma CI foi executada;
M7 segue em andamento e QA visual/nativa por OS continua pendente.

**Frescor da amostra i18n M7 (2026-11-07):** estados “aguardando primeira
amostra”, “menos de um segundo”, idade em segundos/minutos e “sem amostra
recente” agora vêm de `[sample-age]` nos catálogos pt-BR/en-US. A apresentação
usa `SampleAgeLabel` tipado em vez de construir uma frase em português e depois
reinterpretá-la; templates validam exatamente um `{amount}` e formatam os números
com agrupamento local. O último estado e timestamp são conservados para reaplicar
o rótulo existente ao alternar idioma, sem esperar nova resposta IPC. O teste
Slint confirma a label acessível “Amostra há 1.234 min” → “Sample 1,234 min ago”
→ “Amostra há 1.234 min”; testes unitários cobrem amostra inicial, idade curta,
segundos, minutos, relógio adiantado e desconexão. Gate local em Rust 1.98.1
passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**147 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Nenhuma CI nova ou QA
visual/nativa foi executada; M7 continua em andamento.

**Estados de conexão/telemetria indisponível i18n M7 (2026-11-07):** “Agente
desconectado” / “Agent disconnected”, “Aguardando conexão com o agente…” / “Waiting
for agent connection…” e os fallbacks de GPU/bateria indisponíveis estão
centralizados em `[connection-status]`. O texto de espera antes estava duplicado
na seção de processos; agora essa página e os estados de Storage, Network e
métricas usam a mesma entrada. Defaults Slint dos estados foram limpos e a GUI
aplica o catálogo no início, no erro IPC e na troca de idioma, relocalizando os
estados já visíveis sem outra consulta. O teste Slint verifica os labels acessíveis
de desconexão, GPU, bateria e páginas Storage/Network após alternar pt-BR → en-US
→ pt-BR; teste de catálogo valida as traduções. Gate local em Rust 1.98.1 passou:
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**148 aprovados, 0 falhas,
1 teste manual ignorado**) e `cargo build --workspace`. Sem CI nova ou QA
visual/nativa; M7 permanece em andamento.

**Fallbacks e resumos de telemetria i18n M7 (2026-11-07):** os textos de dado
indisponível, amostra indisponível, nome de núcleo, estado/potência desconhecidos,
percentual de memória e estados de swap agora vêm de `[telemetry-status]` nos
catálogos pt-BR/en-US. O caminho de snapshots usa os catálogos para CPU/memória,
bateria e núcleos; `process_start_age_label` consulta o catálogo para timestamps
ausentes. Memória e swap passam a montar seus resumos diretamente no idioma ativo;
foi removido `localize_summary`, que reinterpretava frases em português com
substituições. A validação exige exatamente um `{number}` e `{percentage}` nos
templates. Testes de catálogo cobrem ambos os idiomas e agrupamento de 1.234; o
teste Slint verifica rótulos acessíveis de indisponibilidade e de núcleo ao
alternar pt-BR → en-US → pt-BR. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 benchmark manual ignorado**) e `cargo build --workspace`. `git diff --check`
também passou. Não houve CI nova nem QA visual/nativa; M7 permanece em andamento.

**Idade relativa dos processos i18n M7 (2026-11-07):** rótulos de segundos,
minutos, horas e dias passaram para `[process-age]`, com formas singular/plural
inglesas e separadores numéricos locais. `process_start_age_label` escolhe um
`ProcessAgeUnit` tipado e delega a apresentação aos catálogos; todos os sete
templates validam exatamente um `{amount}`. Testes verificam singular/plural,
`1.234`/`1,234`, timestamp ausente e relógio anterior ao início. A integração
Slint confirma o detalhe acessível do processo em inglês (“1 min ago”) e, após
alternar para pt-BR, em português (“há 1 min”). Gate local passou: `cargo fmt
--all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**150 aprovados, 0 falhas,
1 teste manual ignorado**), `cargo build --workspace` e `git diff --check`. Nenhuma
CI ou QA visual/nativa foi executada; M7 continua em andamento.

**Tipo de interface de rede i18n M7 (2026-11-07):** os valores `Loopback` e o
fallback `Unknown`/`Desconhecido` foram movidos para `network.kind-loopback` e
`network.kind-unknown` nos catálogos pt-BR/en-US. A conversão de cada linha no
worker usa o catálogo ativo em vez de uma ramificação por `Locale`. Teste de
catálogo verifica as quatro combinações; a integração Slint verifica os labels
acessíveis `lo, Loopback` e `enp-test, Desconhecido` em pt-BR, e `lo, Loopback` e
`enp-test, Unknown` após trocar para en-US. Gate local passou: `cargo fmt --all
-- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**150 aprovados, 0 falhas,
1 teste manual ignorado**), `cargo build --workspace` e `git diff --check`. Nenhuma
CI ou QA visual/nativa foi executada; M7 permanece em andamento.

**Resumos acessíveis dos gráficos i18n M7 (2026-11-08):** títulos de CPU/memória,
estado sem amostras e resumo com contagem, mínimo, máximo e valor mais recente
agora usam templates de `[charts]` pt-BR/en-US, com validação de exatamente um
token por variável. O estado compartilhado conserva os pontos efetivamente
exibidos e as faixas selecionadas; o callback de idioma refaz os dois resumos a
partir desses dados, sem aguardar novo snapshot. Defaults hardcoded nos labels
acessíveis do Slint foram removidos. Testes cobrem séries vazias, valores não
finitos, agrupamento de contagem, ambos os gráficos e alternância pt-BR → en-US →
pt-BR via integração Slint. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**150 aprovados, 0 falhas,
1 benchmark manual ignorado**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; M7 continua em andamento.

**Estados de ciclo de vida do agente i18n M7 (2026-11-08):** textos de conexão,
agente ativo/conectado/indisponível, falha ao configurar início automático e
modos systemd/XDG/LaunchAgent/logon do Windows foram externalizados em
`[agent-status]` pt-BR/en-US. Os seis templates de estado são validados por seus
tokens; erros de IPC e de inicialização continuam preservados como dados, sem
tentar traduzi-los. O worker guarda os parâmetros do último estado e o callback
de idioma reapresenta imediatamente a mensagem no catálogo ativo; o estado inicial
“Conectando ao agente…” também não fica hardcoded no Slint. Testes cobrem todas as
variantes e modos em ambos os catálogos; a integração Slint verifica o label
acessível “Agente indisponível · IPC: socket not found” → “Agent unavailable · IPC:
socket not found” → português novamente. Gate local passou: `cargo fmt --all --
--check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 benchmark manual ignorado**) e `cargo build --workspace`; `git diff --check`
também passou. Não houve CI ou QA visual/nativa; M7 permanece em andamento.

**Descrições acessíveis da ordenação de processos i18n M7 (2026-11-08):**
descrições dos estados não ordenado, crescente e decrescente foram movidas para
`processes.sort-*-description` nos catálogos pt-BR/en-US. `apply_process_texts`
passa os três textos por propriedades do `MainWindow` aos cabeçalhos Slint; o
componente não decide mais idioma nem contém frases inline. Teste de catálogo
confere os seis valores; integração Slint verifica os três estados em pt-BR e
en-US e confirma pt-BR após a alternância. Gate local passou: `cargo fmt --all --
--check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa. A auditoria M7 ainda encontrou os defaults
inline “Aguardando primeira amostra” nos status iniciais de CPU, memória e swap;
M7 segue em andamento.

**Defaults iniciais de telemetria i18n M7 (2026-11-08):** os status iniciais de
CPU, memória e swap deixaram de ter texto literal no Slint. A inicialização da
janela preenche os três com `sample-age.waiting-first-sample`, já existente nos
catálogos pt-BR/en-US, sem duplicar a mesma mensagem em outra seção. O teste de
integração Slint alterna pt-BR → en-US → pt-BR e verifica os três valores da
janela em cada idioma. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; a auditoria M7 continua.

**Label acessível das linhas de processo i18n M7 (2026-11-08):** o texto
“memory”/“memória” usado na descrição acessível de cada processo foi movido para
`processes.memory-accessible-label` nos catálogos pt-BR/en-US. `apply_process_texts`
passa a propriedade ao Slint, removendo o ramo de idioma e as duas frases inline;
a integração Slint confirma a descrição da linha nos dois idiomas após alternar
pt-BR/en-US. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; M7 permanece em andamento.

**Rótulos técnicos da tela de processos i18n M7 (2026-11-08):** os rótulos `PID`,
`PPID` e `CPU` foram movidos para `processes.pid-label`,
`processes.parent-pid-label` e `processes.cpu-label` nos catálogos pt-BR/en-US.
Embora os valores atuais sejam abreviações iguais nos dois idiomas, cabeçalhos,
detalhes do processo selecionado e labels acessíveis agora usam as propriedades
catalogadas, sem strings inline no Slint. Testes de catálogo cobrem os dois
idiomas; a integração verifica cabeçalhos, identidade/detalhes e linhas acessíveis
após alternância de idioma. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; M7 continua em andamento.

**Rótulo PID da árvore de processos i18n M7 (2026-11-08):** o texto `PID` que
formatava cada linha da árvore de processos deixou de ficar embutido em Rust.
`process_tree_rows` recebe `processes.pid-label` do catálogo ativo; teste cobre a
formatação e a ordem/profundidade da árvore para pt-BR e en-US. Gate local passou:
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; M7 permanece em andamento.

**Nomes acessíveis dos seletores de idioma i18n M7 (2026-11-08):**
`Português (Brasil)` e `English (United States)` foram movidos das propriedades
inline do Slint para `header.language-*-accessible` nos dois catálogos. Os nomes
próprios continuam em sua forma nativa em ambos os locales. Teste de catálogo
verifica os quatro valores; integração Slint verifica os nomes acessíveis antes
e depois da seleção en-US. Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; auditoria M7 e plano M0–M8 permanecem abertos.

**Abreviações visíveis dos seletores de idioma i18n M7 (2026-11-08):** os rótulos
`PT` e `EN` saíram dos literais no Slint e agora vêm de
`header.language-pt-br-short` e `header.language-en-us-short` nos catálogos
pt-BR/en-US. As abreviações continuam iguais nos dois locales; o teste de
catálogo verifica ambos e a integração exercita os seletores ao trocar de idioma.
Gate local passou: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**149 aprovados, 0 falhas,
1 verificação manual ignorada**), `cargo build --workspace` e `git diff --check`.
Não houve CI nem QA visual/nativa; auditoria M7 e plano M0–M8 permanecem abertos.

**Varredura de literais Slint M7 (2026-11-08):** após externalizar `PT`/`EN`, a
busca por strings restantes em `apps/desktop/ui/*.slint` encontrou apenas nomes
de import, chaves/valores de protocolo e seleção interna, códigos de locale e
símbolos padronizados de unidade (`MB`, `MiB`, `SI`, `IEC`, `%`); não identificou
outra frase traduzível inline nesta camada. Isso encerra apenas esta passagem
estática de textos Slint, não o aceite M7 de acessibilidade/teclado, QA visual e
nativa, estabilidade ou hardware.

**Auditoria estática de contraste M7 (2026-11-08):** calculada a luminância
relativa sRGB para os pares efetivamente usados no `main.slint` (superfícies
neutras, seleção de navegação/processo e seletores ativos, texto secundário,
estados do agente e acentos textuais das métricas). As combinações amostradas
atingiram AA para texto normal (mínimo **4,98:1**, `#8190a2` em `#19212b`, nos
rótulos do eixo do gráfico); outros pequenos rótulos relevantes ficaram em
5,18:1 (`#8493a5` em `#19212b`) e 5,28:1 (`#8292a5` em `#171e27`). Não foi
necessária mudança de cor nesta fatia. É uma verificação numérica de cores
estáticas, não avaliação de screenshot, contraste de bordas/foco (critério
não textual), paleta light/alto contraste, variação de monitor/tema ou teste por
leitor de tela; QA visual e acessibilidade assistiva continuam pendentes.

**Persistência da densidade M7 (2026-11-08):** confortável/compacta agora
compartilham `ui-settings.toml` com SI/IEC, mantendo confortável e IEC como
defaults. `DensityPreference` valida somente os dois valores já oferecidos; o
arquivo antigo sem `density` continua válido via serde default. Testes de escrita
e leitura confirmam que mudar densidade preserva unidade SI e mudar unidades
preserva densidade compacta; a integração Slint seleciona compacta, recarrega e
aplica as preferências com a mesma função da inicialização, verifica unidades IEC
e densidade e depois seleciona confortável. Falha de gravação também foi testada:
a escolha continua ativa na sessão. ADR-016 registra o escopo e deixa a opção
extra compacta pendente, sem inventar dimensões. Revalidação local em
2026-11-08: fmt, Clippy, build e `SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace
--locked` (**151 aprovados, 0 falhas, 2 verificações manuais ignoradas**);
`cargo audit --no-fetch` sem vulnerabilidades/yanked, com seis avisos permitidos
de dependências não mantidas. Nenhuma CI nova ou QA nativa foi executada.

**Bloqueios externos de M2 (separados do QA virtual):**

- **USB físico — bloqueado:** requer mídia removível descartável conectada a um
  host de teste autorizado. O `virtio-blk` QMP não substitui esse ensaio.
- **macOS Apple Silicon, Windows 11 x64 e Linux ARM64 — bloqueados para runtime:**
  requerem máquinas/sessões nativas; CI de compilação não prova enumeração,
  semântica de I/O, hotplug ou comportamento visual nesses sistemas.
- **QA visual nativa M7 neste host — bloqueada:** a sessão atual é `tty`, sem
  `DISPLAY`/`WAYLAND_DISPLAY`; o probe read-only não encontrou KWin, Xvfb,
  ferramentas de captura/entrada nem workspace gráfico privado. Nenhuma sessão
  do usuário foi acessada. A QA virtual Ubuntu 24.04/Xvfb com screenshots e
  interação real em guest isolado está registrada acima e cobre apenas parte da
  jornada; ela não substitui screenshots/testes nativos por OS. Para isso, é
  necessário workspace gráfico privado aprovado ou host de QA com captura e
  entrada disponíveis; testes Slint de integração também não substituem essa
  evidência.

**Regressão histórica de atualização da GUI:** após o histórico completar a
janela, uma implementação antiga da localização do rótulo `−5 min` panica ao
cortar a string UTF-8 pelo byte 1, encerrando o worker IPC da janela e congelando
os modelos. A correção daquela revisão usou `strip_prefix('−')`; a implementação
vigente, registrada acima, substituiu a tradução baseada em strings por um estado
tipado e templates catalogados, eliminando esse caminho de corte. Na QA original,
um ensaio de escrita atualizou taxas/capacidade, remoção/reconexão refletiu-se na
tela e o log não registrou novo panic.

**Gate local após a correção UTF-8 (2026-11-07):** passaram `cargo fmt --all --
--check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**94 aprovados, 0 falhas,
1 benchmark manual ignorado**) e `cargo build --workspace`.

**Gate local após a classificação (2026-11-07):** passaram novamente
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**100 aprovados, 0 falhas,
1 benchmark manual ignorado**) e `cargo build --workspace`. Nenhum CI novo foi
executado para estas alterações locais.

**Gate local após a auditoria M3 GPU e adapter AMD experimental (2026-11-07):** passaram
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace` (**114 aprovados, 0 falhas,
1 benchmark manual ignorado**) e `cargo build --workspace`. A suíte de GUI foi
mantida em uma única inicialização do backend Slint por processo; nenhuma CI nova
foi executada. A validação física NVIDIA registrada anteriormente é a única evidência de GPU real; esta fatia não acrescentou homologação AMD ou Intel.

**Limites da evidência:** este é hotplug virtual Linux, não USB físico. Software
rendering em Xvfb não homologa GPU, Wayland, HiDPI, leitores de tela, outras
distribuições nem macOS/Windows. Não é benchmark nem ensaio de capacidade. A
validação nativa de adapters e hotplug físico continuam pendentes; a CI verde
anterior não cobre esta alteração local. Nenhuma captura ou dado privado do host
foi adicionado ao repositório. A identidade local ainda não detecta troca entre
ciclos que reutilize mount, filesystem e nome com counters não regressivos
(ADR-005). M2 permanece em andamento.

**Próxima sequência e dependências para fechar M0–M8:** manter as fatias locais
com gate verificável e atualizar a tabela sem promover marcos por inferência. A
QA virtual Ubuntu, benchmark de renderer software, auditoria de dependências,
SBOM local e changelog já foram registrados; não substituem estes itens:

- **Acesso de QA/hardware:** host nativo macOS Apple Silicon, Windows 11 x64 e
  Linux ARM64; mídia USB descartável autorizada; GPUs reais por família; sessão
  com Orca/VoiceOver/NVDA; e máquina autorizada para soak de 24–72 h, update,
  reboot e sleep/wake. Sem esses recursos, M0–M4 e M7 não podem receber aceite
  nativo/hardware.
- **Decisões do responsável pelo produto/segurança:** confirmar nome/handle
  público para o aviso de copyright e aprovar allow-list após revisar as licenças
  de terceiros; no M5, definir implantação/acesso ao Vault, ciclo de vida das
  credenciais e limites/retenção/recuperação do spool durável por usuário; somente se a fatia M6
  for ampliada, política de helper/outras contas/prioridades e auditoria além do
  sink local; definir instruções de reporte de vulnerabilidades sem assumir SLA
  ou canal privado; respeitar a decisão de distribuir artefatos macOS sem
  assinatura/notarização; preparar outras identidades de assinatura apenas se
  um formato for explicitamente aprovado. Não preencher esses itens por suposição.
- **Release owner:** após as decisões acima, homologar instaladores, upgrade e
  desinstalação por OS, vincular SBOM/checksums/proveniência aos artefatos finais,
  e executar CI remota no branch de entrega. Nenhum commit, push, release ou
  operação remota foi autorizado nesta continuação.

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

**Entregas:** empacotamento macOS sem assinatura/notarização conforme decisão do usuário; Windows installer (assinatura Authenticode apenas se habilitada em decisão própria), Linux pacotes, docs, CONTRIBUTING, SECURITY, SBOM, changelog, checksums, matriz de hardwares testados, screenshots reais e benchmark publicado.

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

**Todos os limites abaixo são *targets provisórios de aceitação*.** Resultados medidos
estão identificados abaixo e não promovem por si só o aceite multiplataforma.
Normalizar uso de CPU por núcleo lógico; registrar memória RSS, PSS onde possível,
alocações, disco e wakeups.

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

**Repetição local (2026-11-08):** três execuções release sequenciais do mesmo
teste, cada uma com 10 aquecimentos e 100 amostras, resultaram em p95 de
**1,015 ms**, **1,158 ms** e **1,152 ms**, com máximos de **1,207 ms**,
**1,214 ms** e **1,165 ms**. As três ficaram abaixo do limite de 150 ms do teste.
Ambiente: Linux x86_64, kernel `6.12.113+deb13-amd64`, Rust 1.98.1, Intel Xeon
E5-2670 v2 @ 2.50 GHz, 20 CPUs visíveis. O resultado mede apenas query do agente
e codificação protobuf para a fixture de 10.000 processos e resposta de até
1.000; não mede transporte completo, pintura, input-to-frame ou p95 da GUI e não
substitui profiling/renderização nativa requeridos pelo aceite M1.

**Ciclo de vida visual M1 em VM (2026-11-08):** em Ubuntu 24.04 x86_64/X11
isolado, o `monitorctl processes --limit 1000` confirmou o fixture
`nsm-ui-live-qa`; a GUI QA exibiu a linha com filtro correspondente. Após
encerramento controlado do PID sintético, o agente deixou de reportá-lo e a
captura da GUI mostrou `0 processes visible` e o estado vazio, sem fechar ou
reiniciar a janela. Em seguida, somente a GUI QA desta rodada foi encerrada após
verificar PID e caminho exato do executável; `monitorctl status` confirmou que o
agente separado continuava `running` no protocolo IPC 1.10. Capturas antes/depois
ficaram fora do repositório em artefatos privados de QA. Isto verifica o ciclo
visual em Linux virtual e a separação da GUI/agente; não substitui instalação
limpa, garantia de mesma instância do agente, PID reuse, screenshots/execução
nativa nos três OS ou reader assistivo real. Nenhum pacote ou permissão foi
alterado.

**Benchmark UI local (2026-11-08):** o teste manual ignorado
`tests::ui_performance_budget_10k_processes_and_1200_chart_points` usa o
`TestingBackend` com rasterizador Slint software real, janela 1120 × 720 e
fixture determinística. Cada cenário executa 10 aquecimentos e 100 medidas; três
campanhas release independentes foram executadas sequencialmente. As medidas
incluem transformação da fixture já preparada, atualização do modelo Slint e
`Window::take_snapshot()` enquanto a página de destino está visível:

| Cenário | p95 nas três campanhas | Maior amostra observada | Budget |
|---|---:|---:|---:|
| Ordenar, converter e renderizar 10.000 linhas | 52,128–52,629 ms | 59,341 ms | ≤150 ms |
| Filtrar 10.000 e renderizar uma correspondência | 18,320–18,581 ms | 18,822 ms | ≤150 ms |
| Selecionar painel e renderizar 1.200 pontos raw | 22,536–22,838 ms | 31,793 ms | p95 ≤100 ms; max <250 ms |
| Selecionar painel e renderizar série downsampled (≤72 pontos) | 18,395–18,551 ms | 25,128 ms | p95 ≤100 ms; max <250 ms |

Ambiente: Linux x86_64, kernel `6.12.113+deb13-amd64`, Rust/Cargo 1.98.1,
Intel Xeon E5-2670 v2 @ 2.50 GHz, 20 CPUs visíveis, profile `--release`.
Reproduzir com:

```sh
SLINT_EMIT_DEBUG_INFO=1 cargo test --locked -p native-monitor-desktop ui_performance_budget_10k_processes_and_1200_chart_points --release -- --ignored --nocapture
```

As três campanhas passaram as asserções dos budgets. O benchmark injeta 10.000
linhas direto na UI (sem limite IPC de 1.000), ordena a tabela inteira, filtra
para uma correspondência e inclui conversão das linhas/modelo. O caso raw do
gráfico estressa 1.200 elementos; o caso downsampled percorre `select_history`
com o limite de 72 pontos usado pela aplicação. Fixture cloning, ciclo IPC,
digitação real, despacho pelo event loop nativo, apresentação do frame pelo SO,
GPU física, CPU/RSS e freeze da janela de produção não são medidos; o tempo
sincrônico de snapshot é uma proxy restrita, não prova input-to-frame. `perf`,
`samply` e ferramentas FlameGraph não estão instaladas neste host; nenhum
profiler, permissão ou pacote foi alterado. Probe read-only em 2026-11-08:
EUID 1000, `/proc/sys/kernel/perf_event_paranoid=3`, BTF do kernel presente;
sem ferramenta de captura e sem autorização para alterar permissões/sysctls,
profiling fica bloqueado neste host. Portanto, o target passou no rasterizador
virtual para essa fatia, mas profiling/flamegraph e validação visual de
responsividade em backends/OS nativos permanecem pendentes.

**Revalidação do benchmark após persistência M7 (2026-11-08):** três novas
execuções sequenciais do mesmo comando, no mesmo Linux x86_64, kernel
`6.12.113+deb13-amd64`, Rust 1.98.1, Intel Xeon E5-2670 v2 @ 2.50 GHz e 20 CPUs
visíveis, passaram todas as asserções. p95/max observados: ordenar/renderizar
10.000 linhas **51,669–52,734 / 74,420 ms**; filtrar/renderizar **16,749–16,871 /
23,521 ms**; gráfico raw de 1.200 pontos **21,062–21,512 / 22,264 ms**; série
downsampled **16,741–17,334 / 17,791 ms**. Máximos seguem abaixo dos budgets
correspondentes. O teste continua limitado ao renderer Slint software e não
fecha o critério de input-to-frame, profiler ou QA nativa.

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
- **Distribuição:** verificar abertura/Gatekeeper para os pacotes macOS sem assinatura escolhidos, upgrade/rollback, limpeza uninstaller e pacote em máquina limpa sem Rust/toolchain ou driver SDK instalados. Revogação de helper só se um helper vier a ser adotado.

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

- **Licença de produto:** MIT foi aprovada pelo usuário em 2026-11-09 e substitui a proposta inicial GPL-3.0-only; o metadado `Cargo.toml` declara MIT e `LICENSE` contém `Copyright (c) 2026 Lucas Eufrasio`, conforme autorização do titular para usar a autoria dos commits. A allow-list das licenças encontradas nas dependências está aprovada e registrada em ADR-018, com gate `cargo-deny`; isso não constitui parecer jurídico nem autoriza redistribuir SDKs/binários de terceiros por inferência.
- Incluir LICENSE, SPDX headers, NOTICE/third-party notices quando aplicável, SBOM e política de contribuição; exemplos e documentação com licenças explicitadas.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, templates Issue/PR, guidelines de hardware report, roadmap público e definição clara de compatibilidade experimental.

### 16.2 Empacotamento por OS

| OS | Artefato V1 | Inicialização do agente | Cuidados |
|---|---|---|---|
| macOS arm64 | `.app` + `.dmg` sem assinatura/notarização, conforme decisão do usuário (2026-11-09) | LaunchAgent do usuário; daemon opt-in para system mode | Verificar e documentar avisos/restrições do Gatekeeper; não contornar proteções; validar que o modo de distribuição sem assinatura funciona nas versões alvo. |
| Windows 11 x64 | `.msi` via WiX ou instalador equivalente; zip portátil sem serviço como alternativa limitada | Agente via tarefa de logon do usuário; Windows Service LocalService só no modo sistema opt-in | Assinatura Authenticode quando disponível; não instalar serviço em modo portátil sem permissão. |
| Linux x86_64 | `.deb`, `.rpm`, AppImage | systemd --user, helper polkit opcional | deps runtime/versionadas, Wayland/X11, sufixos de pacote e uninstall. |
| Linux aarch64 | `.deb`, `.rpm`, AppImage se homologado | idem | binary/ABI nativo e GPU/sensores explicitamente condicionais. |

**Atualizações:** GitHub Releases com checksum, changelog e notificações opcionais; artefatos macOS sem assinatura/notarização conforme decisão do usuário. Updater automático desabilitado por padrão em V1; downloads só via HTTPS e verificação de checksum/proveniência do artefato; migração de configuração compatível e recuperação/rollback. Instalação headless documentada para usuários técnicos. Sem App Store.

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

**Épico E11 — Release:** build, packaging, update/uninstall, SBOM, docs, validated hardware matrix and performance report; keep macOS artifacts unsigned per the current decision.

**Template de issue:** objetivo, plataforma/dispositivo, fonte de métrica, pré-condições/permissões, UI esperada, payload IPC, limites de privacidade, testes automatizados, teste em hardware real, benchmark delta, documentação e critérios observáveis.

## 18. Critérios de pronto (DoR), concluído (DoD) e gates de release

**DoR de feature:** comportamento definido, valores/semântica/documentação da API, fonte por OS, forma de detectar suporte, fluxo sem permissão, threat impact, plano de benchmark e hardware de teste identificados.

**DoD de feature:** funcionalidade real de ponta a ponta, sem dados mock em release; testes unit/integration/E2E pertinentes; fonte/unidade/qualidade no contrato; erro/stale/unsupported demonstráveis; performance medida; docs/changelog/ADR atualizados; check acessibilidade. Feature pode ter suporte parcial se rotulada como tal e sem quebra das outras áreas.

**Gate V0.1 (foundation):** GUI + agent + IPC + CPU/RAM/processes em três plataformas; persistência após GUI fechar; protótipos UX e testes iniciais.

**Gate V0.5 (hardware alpha):** discos e GPU familiares demonstrados, sensores e energia condicionais, SMART seguro, capability-report completo, sem crashes de backend individual.

**Gate V0.8 (observabilidade beta):** exporter e queue confiáveis, ações seguras, serviço/instalador, i18n, alertas, suporte macOS arm64/Linux x64+arm64/Windows11 x64 com limitações honestas.

**Gate V1.0 (release):** todos os fluxos centrais com testes repetíveis, checksums/proveniência e assinaturas somente nos formatos que o projeto tenha aprovado, artefatos macOS sem assinatura conforme a decisão atual, hardware matrix, benchmark real contra referências, security review, filas e export com perda explícita, documentação para instalação/recuperação/desinstalação. Falta de hardware para família implica badge *experimental* ou adiamento de claim; **nunca** “suporte completo” por compilação apenas.

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
