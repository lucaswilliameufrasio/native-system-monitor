# Contribuindo

## Ambiente

- Rust 1.98.1, instalado por `rust-toolchain.toml` com `rustfmt` e `clippy`.
- O workspace é dividido entre `apps/` (GUI, agente e CLI), `crates/`
  (domínio, coletores, IPC, configurações e exportadores), `proto/` (contratos
  IPC) e `docs/` (plano e ADRs).
- Linux precisa de `libfontconfig1-dev` para compilar Slint, conforme o CI.

## Antes de enviar uma alteração

Execute o mesmo gate usado no CI, na raiz do repositório:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace
cargo build --workspace
```

`SLINT_EMIT_DEBUG_INFO=1` é necessário para os testes de interação da GUI. O
teste de desempenho IPC de 10 mil processos é manual e ignorado na suíte padrão;
rode-o em release quando a alteração afetar consultas/ordenação de processos:

```sh
cargo test -p native-monitor-ipc --release filter_sort_10k_processes_p95 -- --ignored --nocapture
```

## Ensaios opt-in

Os ensaios Remote Write iniciam containers locais descartáveis e usam somente
loopback:

```sh
scripts/test-victoriametrics-remote-write.sh
scripts/test-prometheus-remote-write.sh
```

Eles requerem Docker com Compose e `curl`, não devem apontar para destinos
compartilhados ou de produção e recusam execução se já houver um agente ativo
para o usuário. Seus containers e redes próprios são removidos ao terminar.

Em Linux, `scripts/validate-linux-network.py` compara o tráfego de loopback
gerado localmente (limitado a 32 MiB) com os deltas de `/proc/net/dev` e as
leituras de `monitorctl devices --type network`. Requer um agente já ativo e
alcançável; não inicia, encerra nem reconfigura agente algum. Por exemplo:

```sh
python3 scripts/validate-linux-network.py --size-mib 8
```

O ensaio é uma checagem limitada de tendência/unidade, não um benchmark de
capacidade, e não acessa a rede externa.

## Contratos e segurança

- Preserve campos desconhecidos como ausentes; não invente valores para sensores
  indisponíveis.
- Mudanças incompatíveis em IPC devem atualizar o protocolo e manter testes de
  serialização/compatibilidade.
- Não exponha command lines, caminhos, identificadores privados ou credenciais
  em métricas, logs, fixtures ou documentação.
- Antes de adicionar sinais, privilégios, helper ou autenticação, documente o
  contrato de identidade, autorização, auditoria e recuperação em ADR e teste as
  fronteiras específicas de cada sistema operacional.
- Não inclua tokens, chaves, configurações locais ou dados de máquina em commits.
- O projeto adota a licença MIT; o aviso de copyright e o arquivo `LICENSE`
  devem ser finalizados após confirmação do titular e do ano.
