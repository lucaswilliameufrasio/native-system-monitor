# ADR-008: telemetria NVIDIA opcional via NVML

- **Status:** Aceito para a primeira fatia Linux de M3; outras famílias/OS pendentes
- **Data:** 2026-10-26
- **Referência:** plano mestre, seção 3.5 e incremento M3.

## Contexto

O agente precisa enumerar GPUs e mostrar somente métricas fornecidas pelo
fabricante, sem tornar driver ou GPU requisito para iniciar. A máquina de
desenvolvimento tem NVML disponível, mas CI e outros hosts podem não ter driver.

## Decisão

- Isolar NVIDIA em um crate de collector; carregar NVML dinamicamente pelo
  `nvml-wrapper` e não fazer link obrigatório com bibliotecas proprietárias.
- Inicialização ausente/falha significa capability indisponível; falha parcial
  de uma métrica mantém somente aquele campo ausente. Nenhuma métrica opcional
  vira zero. UUID é a identidade estável do dispositivo.
- Expor uso, VRAM, temperatura, clocks e potência quando NVML suportar. Consultas
  são somente leitura; não mudar clocks, fan, power limit ou estado da GPU.
- Preservar dispositivos válidos quando outro dispositivo falhar e manter o
  estado degradado visível na GUI e CLI junto às métricas parciais; a existência
  de alguma linha válida não transforma a amostra inteira em sucesso.
- O backend é Linux/NVIDIA nesta fatia. Não inferir suporte AMD, Intel ou Apple;
  cada backend precisa de adapter e validação próprios.
- Publicar capability separada por família planejada (`gpu_nvidia`, `gpu_amd`,
  `gpu_intel`, `gpu_apple_silicon`). Famílias sem adapter aparecem explicitamente
  como `unsupported` com fonte `no native adapter`; AMD tem adapter Linux
  experimental, mas continua `unsupported` até homologação em hardware real
  ([ADR-013](013-linux-amdgpu-sysfs.md)); Intel Linux tem adapter experimental,
  mas continua `unsupported` até homologação real ([ADR-014](014-linux-intel-gpu-telemetry.md)).
  Capability informa prontidão validada, não a ausência de hardware no equipamento.

## Alternativas

- Link obrigatório contra NVML: rejeitado porque binários falhariam em hosts sem
  driver NVIDIA.
- Executar `nvidia-smi` em cada amostra: rejeitado por custo de processo e parsing
  textual; NVML fornece API tipada carregada uma vez.
- Um collector genérico que identifica fabricante por nome: rejeitado por
  misturar contratos de sensores com semânticas diferentes.

## Consequências e revisão

O crate depende da biblioteca de bindings, mas a biblioteca NVML do driver é
opcional em runtime. A presença de uma GeForce MX230 permite validação local da
família NVIDIA; ainda faltam validação em outros modelos e adapters AMD/Intel,
Windows e Apple Silicon. Revisar se o wrapper deixa de suportar a versão mínima
de NVML distribuída pelos drivers-alvo.
