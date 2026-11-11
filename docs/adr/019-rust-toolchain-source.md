# ADR-019: mise como fonte única do toolchain Rust

- **Status:** Aceita pelo responsável do projeto em 2026-11-11
- **Data:** 2026-11-11
- **Referências:** `mise.toml`, `.github/workflows/ci.yml`, `README.md`, `CONTRIBUTING.md` e [jdx/mise-action v5.2.7](https://github.com/jdx/mise-action/releases/tag/v5.2.7)

## Contexto

O repositório tinha versões diferentes em `mise.toml`, `rust-toolchain.toml` e
`.github/workflows/ci.yml`. Além da divergência entre versões, a configuração de
`rustfmt` e `clippy` também precisava ser mantida em mais de um lugar. Isso tornava
ambíguo qual toolchain um desenvolvedor ou runner usaria.

## Decisão

- `mise.toml` é a fonte única da versão do toolchain Rust e dos componentes
  adicionais necessários (`rustfmt` e `clippy`).
- Remover `rust-toolchain.toml`; desenvolvimento local usa `mise install` e
  `mise exec -- <comando>` (ou uma shell ativada pelo mise).
- O CI instala as ferramentas a partir de `mise.toml` com `jdx/mise-action`
  (`v5.2.7`, SHA completo fixado), sem declarar novamente a versão do Rust no
  YAML.
- `rust-version` em `Cargo.toml` continua sendo o mínimo compatível da API Rust,
  não um segundo pin do toolchain.

## Alternativas

- **Manter `rust-toolchain.toml` e sincronizar manualmente os pins:** rejeitada,
  pois preserva a duplicidade e permite divergência futura.
- **Manter o toolchain do CI independente do mise:** rejeitada, pois CI e
  desenvolvimento local poderiam compilar com versões diferentes.

## Consequências e revisão

Alterações na versão ou nos componentes Rust devem ser feitas em `mise.toml`; o
CI e os comandos documentados consomem essa configuração. Desenvolvedores e CI
precisam ter mise disponível. Reavaliar se o projeto deixar de usar mise ou se o
CI não puder executar o gerenciador com segurança.
