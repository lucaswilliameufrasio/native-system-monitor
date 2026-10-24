# ADR-001: Slint como toolkit da GUI

- **Status:** Aceito
- **Data:** 2026-10-23
- **Referência:** `docs/PLANO_DESENVOLVIMENTO_MONITOR_SISTEMA.md`, decisões 1.1 e
  incremento M0.

## Contexto

O produto precisa de uma GUI desktop nativa, sem WebView, com tabelas, gráficos,
teclado e acessibilidade. A escolha do toolkit afeta a ponte Rust/UI e o
empacotamento por plataforma.

## Decisão

Usar Slint para a GUI e manter modelos Rust separados da camada visual. A adoção
fica condicionada aos gates de tabela de 10 mil linhas, gráfico de 1.200 pontos,
acessibilidade e renderer alternativo descritos no plano.

## Alternativas consideradas

- **Iced/GPUI:** permanecem alternativas caso o spike reprodutível demonstre que
  Slint não atende aos gates. Não serão escolhidas apenas por preferência
  estética.
- **WebView:** rejeitada por contrariar a decisão de interface nativa do produto.

## Consequências e revisão

O desktop usa `.slint` e `slint::include_modules!()`. Componentes e modelos devem
ser avaliados com benchmarks e testes de acessibilidade reais. Reabrir a decisão
se um gate obrigatório falhar, registrando medições e impacto de migração.
