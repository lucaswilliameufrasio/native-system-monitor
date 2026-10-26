# ADR-012: processo e árvore de descendentes por identidade no IPC

- **Status:** Aceito para a primeira fatia somente de leitura de M6
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 3.4 e M6; ADR-002 e ADR-003.

## Contexto

M6 pede inspeção detalhada e relação pai/filhos. O agente já mantém um snapshot
com `pid`, `start_time_unix_seconds` e `parent_pid`; a consulta geral pode ser
limitada e ordenada, portanto não garante que o processo escolhido nem todos os
descendentes apareçam na resposta. O protocolo IPC é versionado e já restringe
clientes ao usuário da sessão.

## Decisão

- Adicionar uma operação de consulta `ProcessTree`, identificada por PID e hora
  de início exatos. A consulta usa somente um snapshot coletado, não inicia novas
  sondagens e devolve a raiz mais seus descendentes presentes naquele snapshot.
- Construir a relação via PPID do mesmo snapshot; ordenar filhos por PID e
  percorrer em largura. Incluir a raiz primeiro, aplicar o limite configurado e
  respeitar o teto IPC existente. Se identidade não corresponder, responder
  `process_not_found` em vez de selecionar um PID reutilizado.
- Reutilizar `Snapshot.processes` no response, sem expor linha de comando,
  caminho, UID ou outros dados sensíveis ainda não necessários para esta fatia.
  O CLI imprime identidade e árvore com indentação; a GUI continua somente com
  o painel atual nesta primeira entrega.
- A consulta é somente leitura. Não cria autorização para terminar processos ou
  acessar processos de outra conta; sinais e helpers exigem uma decisão própria
  sobre identidade atômica, permissões por OS, confirmação e auditoria.

## Alternativas

- Filtrar a lista genérica de processos no cliente: rejeitado porque o teto e a
  ordenação podem excluir a raiz ou seus descendentes.
- Selecionar somente por PID: rejeitado por permitir que um PID reutilizado
  identifique processo diferente.
- Incluir command line e caminho agora: adiado por privacidade e porque a fonte
  atual não fornece política de consentimento/mascaramento.

## Consequências e revisão

Adiciona campos/operação ao contrato protobuf; cliente e agente negociam versão
minor e clientes antigos continuam sem essa operação. O resultado reflete uma
amostra instantânea: filhos podem nascer/sair durante a renderização e a árvore
não é uma fotografia transacional do kernel. A consulta é limitada por
`max_processes_per_response`; uma árvore truncada deve ser indicada ao usuário.
Antes de ações destrutivas, revisar a segurança específica de sinais e validar
PID reuse atomicamente em cada sistema operacional suportado.
