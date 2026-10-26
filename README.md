# Native System Monitor

Monitor de sistema nativo para desktop, escrito em Rust e com interface Slint.
O produto ainda está em desenvolvimento; este primeiro slice executável coleta
CPU, memória e processos no agente e expõe os dados à GUI e à CLI por IPC local.

## Status

- A GUI e o agente são processos separados. Na primeira abertura, a GUI registra
  o agente no gerenciador da sessão do usuário e o inicia se necessário; fechar a
  janela não encerra a coleta.
- O início no login usa LaunchAgent no macOS, `systemd --user` no Linux (fallback
  XDG autostart sem reinício automático após falha) e tarefa de logon no Windows.
  Não instala serviço de sistema nem exige root/admin. Remoção do registro no
  uninstall depende do empacotamento futuro.
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

Depois de `cargo build --workspace`, execute a GUI; na primeira abertura ela
registra/inicia o agente pela sessão do usuário. A CLI pode ser usada
separadamente para diagnóstico:

```sh
cargo run -p native-monitor-desktop
cargo run -p native-monitorctl -- status
cargo run -p native-monitorctl -- capabilities
cargo run -p native-monitorctl -- processes --sort cpu --limit 25
cargo run -p native-monitorctl -- devices --type disk
cargo run -p native-monitorctl -- devices --type block
cargo run -p native-monitorctl -- devices --type gpu
cargo run -p native-monitorctl -- devices --type battery
```

Para validar tendências de I/O físico Linux contra `iostat` (sysstat), há um
ensaio manual opt-in e limitado. Ele precisa de um agente já ativo e de um
diretório escolhido pelo operador no filesystem que deseja testar; escreve e
lê um arquivo temporário (64 MiB por padrão, máximo 256 MiB) e o remove ao sair:

```sh
scripts/validate-linux-block-io.sh --directory /caminho/no/filesystem
```

Revise o espaço livre e o destino antes de executar. O ensaio não instala
dependências, não pede privilégios e não substitui validação nativa em cada OS.

Em desenvolvimento, o registro aponta para os binários em `target/debug`; apagar
ou mover esse diretório invalida o início automático registrado. Para um teste
manual isolado do IPC, ainda é possível iniciar `cargo run -p
native-monitor-agent -- run`, mas isso não é necessário no fluxo normal da GUI.

O caminho opcional de configuração é TOML:

```sh
cargo run -p native-monitor-agent -- run docs/settings.example.toml
```

## Estrutura inicial

- `apps/agent`: coleta em cadência única e serve snapshots locais.
- `apps/desktop`: GUI Slint; leitura IPC em worker, sem I/O na thread visual.
- `apps/monitorctl`: comandos de status, capacidades e processos.
- `crates/agent-lifecycle`: registro/início por usuário nos gerenciadores de
  sessão, sem serviço de sistema.
- `crates/domain`: tipos de métricas, identidade de processo e histórico limitado.
- `crates/collector-api` e `crates/collectors-common`: contrato e primeiro
  adaptador baseado em `sysinfo`.
- `crates/ipc` e `proto/native_monitor/v1`: protobuf versionado, framing limitado
  e sockets Unix/named pipes locais.
- `crates/settings`: configuração TOML tipada e validada.

Consulte o [plano mestre](docs/PLANO_DESENVOLVIMENTO_MONITOR_SISTEMA.md) e os
[ADRs](docs/adr/) para o escopo e as decisões técnicas.
