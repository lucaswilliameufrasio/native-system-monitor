# ADR-018: allow-list de licenças de dependências

- **Status:** Aceita pelo responsável do projeto em 2026-11-09
- **Data:** 2026-11-09
- **Referências:** plano mestre, seção 10.3 e M8; `deny.toml`; inventário CycloneDX local de 2026-11-09

## Contexto

O projeto usa MIT, mas as licenças das dependências precisam de uma política
separada para que a verificação automatizada não seja confundida com a licença
do aplicativo. A união dos 13 SBOMs locais continha 656 componentes de terceiros,
35 expressões SPDX distintas e nenhum componente sem expressão. O inventário
incluiu 14 componentes Slint com opções `GPL-3.0-only` e duas
`LicenseRef-Slint-*`, além de `r-efi 6.0.0` com uma opção LGPL.

## Decisão

- O responsável aprovou todos os identificadores SPDX e as opções listadas no
  inventário de 2026-11-09, incluindo `GPL-3.0-only`, as duas licenças
  `LicenseRef-Slint-*` e `LGPL-2.1-or-later`.
- `deny.toml` lista explicitamente os identificadores observados. Licenças não
  listadas continuam negadas pelo `cargo-deny`; a aprovação não é um allow-all
  para identificadores futuros.
- O CI executará `cargo deny check licenses` no grafo do workspace. A checagem
  automatiza a política de expressões declaradas, mas não substitui revisão dos
  textos/licenças aplicáveis nem constitui parecer jurídico.

## Alternativas

- **Permitir somente licenças permissivas tradicionais:** rejeitada pelo
  responsável para o inventário atual, que aprovou todas as opções encontradas.
- **Adiar a allow-list:** rejeitada; o responsável aprovou a lista atual para
  permitir uma verificação reproduzível no CI.
- **Permitir qualquer licença futura:** rejeitada; novos identificadores devem
  falhar no gate e passar por revisão/aprovação antes de serem adicionados.

## Consequências e revisão

A allow-list reflete a decisão de política do responsável e os metadados
observados no lockfile/SBOM, não uma conclusão de que todas as alternativas são
equivalentes ou de que qualquer modo de distribuição está automaticamente
autorizado. Revisar o ADR quando uma dependência introduzir novo identificador,
quando os termos de um fornecedor mudarem ou quando a distribuição do produto
for alterada.
