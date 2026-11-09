# ADR-017: controle de processos no escopo do usuário

- **Status:** Aceita para a primeira fatia, aprovada pelo usuário em 2026-11-09
- **Data:** 2026-11-09
- **Referências:** plano mestre, seções 3.4, 8.1, 10 e M6; ADR-002, ADR-003 e ADR-012

## Contexto

M6 já oferece consulta somente de leitura à árvore, mas o plano do produto pede
também encerramento gentil e opção forçada com confirmação. O usuário confirmou
que o monitor deve oferecer controle de processos e aprovou esta primeira fatia;
ela não deve obter privilégio adicional nem agir fora das permissões normais do
agente.

## Decisão

- A primeira fatia atua somente sobre a identidade selecionada (PID + horário de
  início) e sob as credenciais atuais do agente. O agente revalida a identidade
  imediatamente antes de cada ação; se o processo saiu ou a identidade mudou,
  não envia sinal. O sistema operacional continua sendo a autoridade final para
  permitir/negar a ação.
- Não instalar helper, não elevar privilégios e não contornar as restrições do
  sistema. Processos que o agente não pode controlar recebem resultado de
  permissão negada, sem tentativa de bypass.
- A primeira ação solicita encerramento gentil. Em Unix, usa `SIGTERM`; no
  Windows, envia `WM_CLOSE` às janelas de topo pertencentes ao processo, com
  envio limitado no tempo. Isso não afirma que o processo encerrou: a GUI observa
  os snapshots seguintes.
- Após 5 segundos monotônicos, se a mesma identidade continuar ativa, a GUI pode
  oferecer encerramento forçado em uma segunda confirmação explícita. A segunda
  operação revalida a identidade; em Unix usa `SIGKILL`, e no Windows usa a
  operação nativa de término sobre um handle da identidade conferida. Nunca
  converter a primeira ação em força automaticamente.
- Registrar localmente, pelo sink de tracing já configurado, hora, PID, início,
  ação e resultado. Não registrar command line, caminho ou argumentos; não enviar
  eventos ao exporter.
- Usar um único estado discriminado para confirmação/estado da ação na GUI; a
  identidade alvo acompanha o estado, e fechar/cancelar não envia ação.
- A verificação de horário imediatamente antes da ação é a garantia comum desta
  primeira fatia. Adaptadores que conseguem vincular a chamada a um handle
  estável devem fazê-lo; não declarar proteção atômica contra PID reuse em OS
  onde a API de sinalização não oferece essa garantia.

## Alternativas

- **Manter M6 somente leitura:** rejeitada pelo usuário e incompatível com a
  jornada de gestão de processos do plano.
- **Instalar helper/elevador para agir sobre outros usuários:** fora desta
  primeira fatia; exige política, threat review, auditoria e decisão própria.
- **Forçar automaticamente após o prazo:** rejeitada; força sempre exige nova
  confirmação explícita.
- **Usar `sysinfo::Process::kill_with` para todos os sistemas:** rejeitada; a
  implementação Windows da dependência usa `taskkill.exe /F` mesmo quando recebe
  um sinal, portanto não implementa encerramento gentil.

## Consequências e revisão

O IPC precisa carregar uma ação tipada e a identidade exata; erros de identidade,
permissão, plataforma e envio devem ser observáveis sem expor dados privados. Os
testes destrutivos usarão somente processos-fixture criados pelo próprio teste.
Windows precisa de ensaio nativo do caminho `WM_CLOSE`/força; macOS e Linux
precisam validar identidade e permissões reais. Helper, ações para outras contas,
prioridades e retenção/auditoria além do sink local seguem fora desta fatia.
Revisar se a precisão de identidade disponível em algum OS não for suficiente
para manter o risco de PID reuse aceitável.
