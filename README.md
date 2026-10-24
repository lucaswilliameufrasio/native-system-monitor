# Native System Monitor

Monitor de sistema nativo para desktop, escrito em Rust e com interface Slint.
O produto ainda está em desenvolvimento; este primeiro slice executável coleta
CPU, memória e processos no agente e expõe os dados à GUI e à CLI por IPC local.

## Status

- A GUI e o agente são processos separados. Fechar a janela não encerra um agente
  iniciado separadamente.
- O agente ainda precisa ser iniciado manualmente; instalação e inicialização no
  login entram em etapas posteriores.
- CPU começa como “aquecendo” até existir uma amostra delta válida. Dados
  indisponíveis não são apresentados como zero.
- A lista de processos mostra nome, PID, processo pai, CPU e RAM. Linha de
  comando e outros dados sensíveis não são coletados neste slice.
- Histórico CPU/RAM é mantido em memória no agente, limitado ao número de
  amostras configurado (300 por padrão). É perdido quando o agente é encerrado.
- As plataformas-alvo do plano são macOS Apple Silicon, Linux x86_64/aarch64 e
  Windows 11 x64. A execução local validada neste checkout é Linux x86_64; os
  outros alvos dependem da CI e de testes em máquinas reais.
- A licença final permanece pendente da auditoria indicada no plano.

## Desenvolvimento local

Pré-requisitos: Rust `1.98.1` (definido em `rust-toolchain.toml`) e as
dependências nativas do Slint para a plataforma.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Execute o agente, a CLI e a GUI em terminais separados:

```sh
cargo run -p native-monitor-agent -- run
cargo run -p native-monitorctl -- status
cargo run -p native-monitorctl -- capabilities
cargo run -p native-monitorctl -- processes --sort cpu --limit 25
cargo run -p native-monitor-desktop
```

O caminho opcional de configuração é TOML:

```sh
cargo run -p native-monitor-agent -- run docs/settings.example.toml
```

## Estrutura inicial

- `apps/agent`: coleta em cadência única e serve snapshots locais.
- `apps/desktop`: GUI Slint; leitura IPC em worker, sem I/O na thread visual.
- `apps/monitorctl`: comandos de status, capacidades e processos.
- `crates/domain`: tipos de métricas, identidade de processo e histórico limitado.
- `crates/collector-api` e `crates/collectors-common`: contrato e primeiro
  adaptador baseado em `sysinfo`.
- `crates/ipc` e `proto/native_monitor/v1`: protobuf versionado, framing limitado
  e sockets Unix/named pipes locais.
- `crates/settings`: configuração TOML tipada e validada.

Consulte o [plano mestre](docs/PLANO_DESENVOLVIMENTO_MONITOR_SISTEMA.md) e os
[ADRs](docs/adr/) para o escopo e as decisões técnicas.
