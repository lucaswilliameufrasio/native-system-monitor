# ADR-004: ciclo de vida do agente na sessão do usuário

- **Status:** Aceito
- **Data:** 2026-10-24
- **Referência:** plano mestre, seções 4.1 e M1; ADR-002.

## Contexto

A primeira abertura não pode exigir que a pessoa rode `agent run` ou mantenha um
terminal aberto. Ao mesmo tempo, a GUI não deve ser dona do processo coletor: o
agente precisa continuar após fechar a janela, sem privilégio elevado e sem
duplicar a coleta.

## Decisão

Usar o gerenciador de sessão do próprio usuário, sem serviço de sistema:

- macOS: LaunchAgent com `RunAtLoad` e reinício após saída inesperada;
- Linux: `systemd --user` com `Restart=on-failure`; se não houver um gerenciador de
  usuário acessível, registrar autostart XDG. Nesse fallback, a GUI inicia uma
  vez o processo destacado nesta sessão; autostart futuro funciona, mas o
  reinício automático após crash não é garantido;
- Windows: tarefa no Agendador de Tarefas para logon interativo, nível mínimo de
  privilégio e tentativas limitadas de reinício após falha.

No primeiro uso, a GUI consulta o IPC. Se o agente já responde, apenas registra
o início futuro; se não responde, registra e solicita ao gerenciador que o inicie.
Em ambos os casos, o gerenciador, e não a GUI, passa a manter a execução. A GUI
continua cliente IPC, exibe erro/estado e oferece retry. Nenhuma etapa exige
admin/root. Arquivos de inicialização são idempotentes e só podem ser atualizados
quando contêm o marcador de propriedade do aplicativo.

## Alternativas consideradas

- **Agente filho supervisionado pela GUI:** rejeitada; encerraria ou acoplaria a
  coleta ao ciclo de vida da janela.
- **Comando manual ou serviço de sistema:** rejeitados como fluxo padrão; o
  primeiro falha na jornada de primeira execução e o segundo exige privilégios
  e continua após logout.
- **Uma única implementação de serviço para todos os OS:** rejeitada porque os
  gerenciadores nativos fornecem a integração de login e as garantias de restart
  apropriadas a cada plataforma.

## Consequências e revisão

O escopo é por usuário e vale enquanto a sessão estiver ativa; execução após
logout/headless continua opt-in e fora deste modo. A instalação de desenvolvimento
registra o caminho absoluto do binário ao lado da GUI; mover ou apagar `target/`
invalida esse registro. O empacotamento futuro deve instalar ambos os binários em
caminho estável e remover apenas os registros do próprio aplicativo na
desinstalação. Rever se o primeiro instalador adotar outro layout ou se testes
reais de OS mostrarem comportamento diferente do gerenciador previsto.
