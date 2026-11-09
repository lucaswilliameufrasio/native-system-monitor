# Native System Monitor

Monitor de sistema nativo para desktop, escrito em Rust e com interface Slint.
O produto ainda está em desenvolvimento; o checkout atual inclui agente
independente, GUI e CLI sobre IPC local, com coleta comum e fatias específicas
de plataforma descritas abaixo. Ainda não é uma release V1 homologada.

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
- A lista de processos mostra nome, PID, processo pai, CPU e RAM. A árvore é
  somente leitura. O encerramento de processos usa as permissões normais do
  agente, pede confirmação para o encerramento gentil e exige uma segunda
  confirmação antes de qualquer encerramento forçado; não há helper elevado.
- No Linux, o snapshot também pode incluir volumes, I/O de dispositivos de bloco,
  interfaces de rede e bateria. NVIDIA é opcional via NVML; adaptadores AMD/Intel
  Linux são experimentais e continuam `unsupported` até homologação em hardware
  real. Ausência de dados não é convertida em zero.
- A GUI oferece pt-BR/en-US, preferências locais SI/IEC e densidade confortável/
  compacta. Preferências de bytes usam IEC e a densidade confortável por padrão.
- Histórico CPU/RAM é mantido em memória no agente, limitado ao número de
  amostras configurado (300 por padrão). É perdido quando o agente é encerrado.
- Prometheus scrape e Remote Write são opcionais e desativados por padrão. A fila
  Remote Write é limitada e volátil; ainda não há integração com Vault/credential
  stores nem spool durável. Use somente receptores que não exijam autenticação.
- As plataformas-alvo do plano são macOS Apple Silicon, Linux x86_64/aarch64 e
  Windows 11 x64. CI para `637ed3b` passou em Ubuntu, macOS e Windows, incluindo o
  gate de licenças no Linux; isso valida os checks automatizados, não substitui QA
  visual nem ensaios de hardware por sistema.
- A licença do projeto é MIT (`LICENSE`, copyright 2026). A allow-list aprovada
  dos identificadores de terceiros está em `deny.toml` e ADR-018; o scanner não é
  parecer jurídico nem aprovação automática de novos identificadores.

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

Para validar tendências de I/O físico Linux contra os contadores independentes
de sysfs, há um ensaio manual opt-in e limitado. Ele precisa de um agente já
ativo e de um diretório escolhido pelo operador no filesystem que deseja testar;
escreve e lê um arquivo temporário (64 MiB por padrão, máximo 256 MiB) e o
remove ao sair:

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
cargo run -p native-monitorctl -- processes --limit 1000
cargo run -p native-monitorctl -- process-tree 1234:1700000000
```

`process-tree` exige PID e hora de início exatos mostrados pela lista; a operação
é somente leitura e sinaliza quando a árvore excede o limite de resposta.

O endpoint de scrape Prometheus é opcional e permanece desativado no exemplo.
Para habilitá-lo, altere `[export.prometheus]` para `enabled = true`; ele só
aceita um endereço loopback, por exemplo `127.0.0.1:9464`. Scrape local pode ser
verificado com `curl http://127.0.0.1:9464/metrics`. `families` aceita uma
allowlist entre `cpu`, `memory`, `storage`, `network`, `gpu` e `battery`; a
família `health` é sempre preservada e lista vazia significa todas. Remote Write
v1 também pode ser habilitado em `[export.remote_write]`; sua fila é apenas em
memória, limitada a 16 MiB por padrão, e o envio não impede a coleta. O destino
exige HTTPS, exceto HTTP em loopback para laboratórios locais. Nesta fatia, ainda
não há integração com credential stores nem spool durável: use somente
receptores que não exijam autenticação até essa integração ser concluída.
Quando o pull também estiver ativo, `health` publica as métricas
`native_monitor_remote_write_*` de habilitação, fila, sucessos, retries,
rejeições, descartes e último sucesso; são contadores/estado voláteis do processo.
O ensaio local reproduzível é `scripts/test-victoriametrics-remote-write.sh`;
ele compila o agente, inicia um VictoriaMetrics descartável em loopback, verifica
a série CPU e remove somente os recursos criados pelo ensaio. Requer Docker
Compose e `curl`. `scripts/test-prometheus-remote-write.sh` exercita também o
receiver nativo do Prometheus (v3.15.1), confirma a série pela API de consulta e
remove o container e a rede isolados ao terminar.

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
