# ADR-013: telemetria AMD Linux por sysfs do amdgpu

- **Status:** Aceito para implementação experimental; capability permanece `unsupported` até homologação em hardware AMD real
- **Data:** 2026-11-07
- **Referências:** plano mestre, seção 3.5 e incremento M3; [ADR-008](008-nvidia-gpu-telemetry.md).

## Contexto

O monitor precisa coletar telemetria AMD no Linux sem tornar AMD SMI ou outro
runtime de fabricante uma dependência obrigatória. O host de desenvolvimento
atual não expõe GPU AMD em `/sys/class/drm`; fixtures podem verificar parsing e
semântica, mas não homologam o driver, o hardware ou a cobertura dos atributos.

## Decisão

- Descobrir apenas entradas DRM `cardN` cujo dispositivo PCI tenha `vendor` igual
  a `0x1002` e link `driver` apontando para `amdgpu`. Não inferir fabricante por
  nome de placa ou pela existência isolada de arquivos de telemetria.
- Usar o endereço PCI BDF resolvido pelo link sysfs como identidade do dispositivo.
  `product_name` é opcional; sem ele, apresentar o nome genérico `AMD GPU`.
- Ler exclusivamente atributos sysfs do kernel, sem comandos auxiliares e sem
  escrita. Fontes e unidades:
  - `gpu_busy_percent`: utilização agregada reportada pelo firmware, percentual
    inteiro validado em 0–100;
  - `mem_info_vram_used` e `mem_info_vram_total`: bytes de VRAM reportados pelo
    amdgpu;
  - `hwmon/temp1_input`: miligraus Celsius, convertido para graus Celsius;
  - `hwmon/freq1_input` e `hwmon/freq2_input`: Hz para clock gfx/compute e memória,
    convertidos para MHz.
- Cada métrica é independente e opcional: campo ausente, inválido ou fora do
  intervalo permanece ausente, nunca zero fabricado.
- Não publicar potência AMD nesta fatia. `power1_average` não é uma fonte segura
  de potência exclusiva da GPU em APUs, pois pode refletir consumo do SoC/CPU.
  Não substituir essa métrica por telemetria semelhante sem semântica documentada.
- A coleta experimental pode alimentar o snapshot para QA local, mas a capability
  `gpu_amd` permanece `unsupported` e informa que a validação de hardware está
  pendente. Fixtures e CI não mudam esse estado; a transição exige teste em GPU
  AMD real e revisão das métricas efetivamente expostas por driver suportado.

## Alternativas

- AMD SMI como requisito: rejeitada nesta fatia para evitar runtime/dependência
  adicional e porque a cobertura depende de driver e hardware.
- Usar `power1_average` como potência da placa: rejeitada por ambiguidade em APUs.
- Identificar dispositivo por nome ou `product_name`: rejeitada por ser opcional,
  não única e insuficiente para distinguir placas iguais.

## Consequências e revisão

O adapter é somente Linux e best-effort por campo. Clocks/temperatura dependem de
hwmon, e VRAM/uso dependem de atributos publicados pelo kernel/firmware. Energia,
memória compartilhada/GTT e métricas específicas de engine ficam fora desta
fatia. Em APUs, o valor de VRAM do driver pode representar um carveout UMA; não
deve ser interpretado como toda a memória compartilhada do sistema. Antes de
marcar `gpu_amd` como suportada, validar descoberta multi-GPU, identidade após
reboot/topologia, unidades e falhas parciais em hardware real.

Referências do kernel consultadas:

- <https://docs.kernel.org/gpu/amdgpu/thermal.html>
- <https://docs.kernel.org/gpu/amdgpu/driver-misc.html>
- <https://docs.kernel.org/gpu/amdgpu/driver-core.html>
