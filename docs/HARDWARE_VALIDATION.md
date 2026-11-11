# Validação voluntária em hardware

Este guia ajuda contribuidores a reunir evidência reproduzível em máquinas reais.
Um relato não muda sozinho o estado de suporte: fixtures, CI e um teste isolado
não homologam uma família de hardware. Em especial, `gpu_amd` e `gpu_intel`
continuam `unsupported` até que a evidência seja revisada e os critérios dos
ADRs [013](adr/013-linux-amdgpu-sysfs.md) e
[014](adr/014-linux-intel-gpu-telemetry.md) sejam atendidos.

## Limites e segurança

- Teste somente em máquina e hardware que você possui ou tem autorização para
  avaliar. Não instale drivers, runtimes ou ferramentas de fabricante apenas para
  este relato sem entender e aceitar o impacto.
- Execute como usuário normal. Não use `sudo`/Administrador, não altere clocks,
  potência, ventoinhas, permissões ou configurações do driver.
- Use a máquina normalmente; não rode stress tests, cargas prolongadas ou testes
  que aqueçam deliberadamente o equipamento. Interrompa se notar instabilidade,
  aquecimento anormal ou comportamento inesperado.
- O guia não solicita linha de comando de processos, arquivos completos de log,
  inventários integrais nem dados de outros usuários. Não publique hostname,
  usuário, serial, UUID, endereço PCI, endereço IP/MAC, ponto de montagem ou nome
  de interface. Redija também caminhos pessoais, nomes de processos e títulos de
  janela antes de compartilhar capturas ou trechos.
- Não inclua tokens, URLs com credenciais, arquivos de configuração pessoal ou
  variáveis de ambiente. O projeto ainda não documenta aqui um canal específico
  para esses relatos; confirme o canal comunitário anunciado pelo mantenedor antes
  de compartilhar. Este guia não cria suporte privado nem SLA.

## Roteiro básico

1. Registre a versão/commit testado e o sistema operacional, versão, arquitetura,
   modelo do dispositivo e versão do driver/runtime relevante. Omita identificadores
   únicos.
2. Inicie a GUI normalmente para que ela inicie ou conecte ao agente. Não inicie
   uma segunda instância do agente.
3. Na raiz do checkout, consulte o status e as capacidades:

   ```sh
   cargo run --locked -p native-monitorctl -- status
   cargo run --locked -p native-monitorctl -- capabilities
   ```

4. Execute somente as consultas pertinentes ao hardware testado. Para GPU:

   ```sh
   cargo run --locked -p native-monitorctl -- devices --type gpu
   ```

   Para outras áreas implementadas, a CLI também oferece `devices --type battery`,
   `disk`, `block` e `network`. As respostas de disco/rede podem conter caminhos,
   nomes locais ou interfaces: não compartilhe a saída integral sem revisar e
   redigir esses campos.

5. Registre uma leitura em repouso e outra durante uma atividade normal e segura
   que use o dispositivo. Aguarde alguns intervalos de coleta entre leituras; a
   cadência padrão do agente é 1 segundo, mas pode ser alterada na configuração.
   Não use uma carga artificial para tentar atingir 100% nem conclua que uma
   métrica está errada por um único valor instantâneo.
6. Compare apenas métricas e unidades que o agente informou como disponíveis.
   Campo ausente não significa zero. Se uma ferramenta de sistema já disponível
   fornecer uma comparação independente e segura, anote o método e a diferença
   observada; não é necessário instalar uma ferramenta para completar o relato.

## Evidência específica de GPU

Inclua no relato, quando aplicável:

- fabricante/modelo comercial, iGPU ou dGPU, sistema/arquitetura e driver;
- versão do runtime (por exemplo, Level Zero para Intel) somente quando já
  estiver instalado e puder ser identificada sem elevar privilégios;
- resultado relevante de `capabilities` e `devices --type gpu`, com IDs únicos
  removidos;
- leituras em repouso e durante uso normal, indicando claramente valores ausentes;
- se o sistema tem mais de uma GPU, quantas entradas foram exibidas e se cada uma
  permaneceu distinguível após reiniciar o aplicativo; não publique UUID/BDF;
- passos para reproduzir e o que você esperava versus o que observou.

Para AMD Linux, potência pode permanecer ausente por decisão de semântica do
adapter. Para Intel Linux, VRAM, clocks, temperatura e potência dependem do
runtime, driver e topologia; consulte o ADR-014 e relate cada campo separadamente.
Não interprete `unsupported` como defeito por si só: essa capability permanece
deliberadamente assim até revisão de homologação.

## Formulário de relato

Copie e preencha apenas os campos pertinentes. Antes de enviar, confira se nenhum
identificador ou dado local foi incluído:

```text
Commit/build testado:
Sistema operacional / versão / arquitetura:
Dispositivo (fabricante/modelo, sem serial/UUID/BDF):
Driver/runtime e versão (se aplicável):
Comando/fluxo testado:
Status e capability relevantes:
Leitura em repouso:
Leitura durante uso normal:
Fonte independente de comparação (opcional):
Passos para reproduzir:
Esperado:
Observado:
Limitações, campos ausentes ou erros:
```

Relatos são evidência para triagem, não declaração de suporte, garantia de
compatibilidade nem autorização para publicar dados além do que o contribuidor
revisou e escolheu compartilhar. A equipe deve registrar a decisão e os critérios
atendidos antes de alterar qualquer capability de `unsupported` para `supported`.
