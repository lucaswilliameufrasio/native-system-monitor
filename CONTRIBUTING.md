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
cargo clippy --workspace --all-targets --locked -- -D warnings
SLINT_EMIT_DEBUG_INFO=1 cargo test --workspace --locked
cargo build --workspace --locked
```

`SLINT_EMIT_DEBUG_INFO=1` é necessário para os testes de interação da GUI. O
teste de desempenho IPC de 10 mil processos é manual e ignorado na suíte padrão;
rode-o em release quando a alteração afetar consultas/ordenação de processos:

```sh
cargo test --locked -p native-monitor-ipc --release filter_sort_10k_processes_p95 -- --ignored --nocapture
```

O benchmark manual da UI também é ignorado na suíte padrão. Ele usa o renderer
software do Slint Testing Backend para medir filtro/ordenação, atualização do
modelo e `take_snapshot` com 10 mil processos sintéticos e gráficos de 1.200
pontos; não mede entrada nativa do SO, IPC de produção ou GPU física:

```sh
SLINT_EMIT_DEBUG_INFO=1 cargo test --locked -p native-monitor-desktop ui_performance_budget_10k_processes_and_1200_chart_points --release -- --ignored --nocapture
```

## Auditoria de dependências e licenças

Execute na raiz para verificar advisories RustSec e a allow-list de licenças
aprovada em [`deny.toml`](deny.toml) / [ADR-018](docs/adr/018-third-party-license-allowlist.md):

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo audit
cargo install cargo-deny --version 0.20.2 --locked
cargo deny check licenses
```

O CI executa `cargo audit` e `cargo deny check licenses` na perna Linux; essas
checagens não são repetidas nas pernas macOS/Windows.

`cargo audit --no-fetch` usa apenas a cópia local da base de advisories e deve
ser tratado como evidência com timestamp, não como garantia de atualização. Os
avisos de dependências não mantidas/yanked devem ser revisados, não silenciados
automaticamente. A allow-list atual reflete os identificadores do inventário de
2026-11-09 aprovados pelo responsável, inclusive as opções GPL/LicenseRef do
Slint e LGPL de `r-efi`; novos identificadores continuam bloqueados até revisão.
O resultado do scanner valida metadados SPDX, não constitui parecer jurídico nem
substitui revisão dos textos e termos de distribuição aplicáveis.

## Inventário CycloneDX

O workspace pode gerar SBOMs JSON CycloneDX 1.5 por pacote, incluindo
dependências dos alvos disponíveis, com `cargo-cyclonedx` 0.5.10:

```sh
cargo install cargo-cyclonedx --version 0.5.10 --locked
cargo cyclonedx --manifest-path Cargo.toml --format json --spec-version 1.5 --target all
```

O comando gera um arquivo `*.cdx.json` ao lado de cada `Cargo.toml` de pacote.
Revise o escopo de features/alvos e os metadados do build antes de distribuir os
arquivos: referências a crates locais podem conter caminhos do ambiente. Esta
geração inventaria dependências; não substitui revisão/aprovação de licenças,
assinatura, checksums ou proveniência de um release.
Na execução de 2026-11-08, a própria ferramenta alertou que `xml-rs 0.8.19` em
seu lockfile está yanked; isso não alterou o lockfile do produto e deve ser
reavaliado antes de colocar o gerador em CI/release.

## Ensaios opt-in

Relatos de métricas em hardware real devem seguir o
[guia de validação voluntária](docs/HARDWARE_VALIDATION.md): rode como usuário
normal, não compartilhe identificadores únicos nem logs integrais e informe
versão/commit, driver/runtime, condições e campos ausentes. Um relato comunitário
é evidência para revisão; não homologa sozinho uma capability.

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
- O projeto adota a licença MIT; o titular confirmou o ano de copyright 2026 e
  autorizou usar o nome de autoria dos commits (`Lucas Eufrasio`) no aviso em
  `LICENSE`. A política aprovada para dependências de terceiros está registrada
  em ADR-018 e é verificada por `cargo deny check licenses`.
