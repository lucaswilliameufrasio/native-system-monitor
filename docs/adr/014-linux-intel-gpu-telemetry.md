# ADR-014: fonte de telemetria Intel Linux via Level Zero Sysman

- **Status:** Adapter Linux experimental implementado; `gpu_intel` permanece `unsupported` até homologação em hardware real
- **Data:** 2026-11-07
- **Referências:** plano mestre, seção 3.5 e incremento M3; [ADR-008](008-nvidia-gpu-telemetry.md); [ADR-013](013-linux-amdgpu-sysfs.md).

## Contexto

M3 precisa de métricas Intel sem tornar uma biblioteca do fabricante requisito
para iniciar o agente e sem misturar estatísticas por processo com telemetria do
dispositivo. Intel Linux tem interfaces DRM distintas para i915 e xe, enquanto
Level Zero Sysman fornece uma API tipada de gerenciamento/consulta e enumera
componentes que podem ou não estar disponíveis em cada driver, dispositivo e
versão de runtime. A máquina de desenvolvimento não foi homologada com GPU Intel;
esta decisão escolhe a fonte, não comprova suporte.

## Decisão

- Usar Level Zero Sysman como fonte primária do adapter Intel Linux, com
  carregamento opcional em runtime. A ausência do loader/driver, falha de
  inicialização, nenhum dispositivo Intel compatível ou permissão insuficiente
  não pode impedir CPU/RAM nem o início do agente. Não adicionar link obrigatório
  a biblioteca Intel nesta decisão.
- Enumerar apenas dispositivos do tipo GPU identificados como vendor PCI Intel
  `0x8086`; não tratar NPU/acelerador de outro tipo como GPU. Preferir UUID do
  dispositivo para identidade; consultar BDF PCI por Sysman quando disponível e
  usá-lo como identidade fallback somente se o UUID for nulo. Não depender de
  nomes de produto para identidade.
- Fazer somente consultas. Não chamar setters de frequência/potência/ventoinha,
  operações de scheduler, reset, overclock ou qualquer API que altere estado.
  Cada campo é independente e best-effort: ausência, erro, valor inválido ou
  semântica não confirmada deixa o campo ausente, nunca zero inventado.
- Semântica de métricas elegíveis:
  - **Utilização:** preferir o grupo `ZES_ENGINE_GROUP_ALL`, definido como todos
    os engines combinados. Calcular percentual por dois snapshots do mesmo handle:
    `100 × ΔactiveTime / Δtimestamp`. As unidades de `activeTime` são específicas
    da implementação e os timestamps só podem ser comparados dentro da mesma
    série/handle. Delta de timestamp zero, regressão, divisão inválida ou
    resultado fora de 0–100% torna a amostra ausente. Não somar grupos individuais
    como substituto: engines podem compartilhar recursos ou executar em paralelo.
  - **VRAM:** somente memória com `zes_mem_properties_t.location == DEVICE` pode
    mapear para VRAM. O total usa `physicalSize` em bytes; o usado pode ser
    derivado de `physicalSize - free` quando ambos são válidos e `free <=
    physicalSize`. `location == SYSTEM` nunca é VRAM. O campo `size` de
    `zes_mem_state_t` está deprecated desde Level Zero 1.15 e não será usado como
    total. Não combinar módulos de memória pai/subdispositivo que possam se
    sobrepor; sem uma partição comprovadamente não sobreposta, deixar VRAM ausente.
  - **Temperatura:** preferir sensores tipados `ZES_TEMP_SENSORS_GPU`; Sysman
    retorna graus Celsius. Sensores globais, de placa, memória ou regulador não
    serão renomeados como temperatura do GPU sem preservar seu escopo.
  - **Clock gráfico:** usar estado atual do domínio `ZES_FREQ_DOMAIN_GPU`, campo
    `actual` documentado em MHz; não confundir frequência solicitada/limite com
    frequência real. Consultar sem alterar faixa ou clock.
  - **Clock de memória:** domínio `ZES_FREQ_DOMAIN_MEMORY` é candidato, mas fica
    fora da primeira exposição até validação por combinação de hardware/driver.
    Há relatos de implementações retornarem MT/s onde consumidores esperam MHz;
    a unidade documentada não basta para justificar conversão universal.
  - **Potência:** `zes_power_properties_t` não identifica por si só o escopo do
    domínio. Só considerar um handle quando a extensão opcional
    `zes_power_domain_exp_properties_t` retornar explicitamente
    `ZES_POWER_DOMAIN_GPU`; domínios `CARD` ou `PACKAGE` podem incluir outros
    componentes e não serão apresentados como watts da GPU. Preferir o contador
    de energia em microjoules e timestamp em microsegundos, calculando watts pela
    razão dos deltas (µJ/µs = W). Se a extensão ou o contador não estiver
    disponível, manter potência ausente. Primeira amostra, erro, delta temporal
    zero, regressão/reset ou permissão ausente também deixa potência ausente.
    Tratar regressão como novo baseline; só calcular wraparound se a largura
    efetiva do contador estiver documentada para aquele runtime/dispositivo. Não
    usar limites de potência como consumo nem agregar domínios de escopo ambíguo.
- DRM `fdinfo` é uma alternativa complementar para futura atribuição por cliente,
  não fonte do uso total do dispositivo: `drm-engine-*` representa tempo ocupado
  por cliente em ns e os dados de memória são alocações por cliente/região.
  Agregar clientes pode omitir descritores inacessíveis e contar buffers
  compartilhados mais de uma vez. Não percorrer `/proc/*/fdinfo` para fabricar
  uma métrica device-wide. sysfs/PMU específicos de i915/xe também não são
  fallback genérico nesta fatia por diferenças de driver, permissões e escopo.

## Alternativas

- **DRM `fdinfo` como fonte primária de uso/VRAM globais:** rejeitada; a
  especificação é por cliente, não por dispositivo, e soma de clientes não é
  necessariamente uma agregação sem dupla contagem ou omissões.
- **Interpretar sysfs de i915 e xe por uma interface única:** rejeitada como
  fonte primária; o ABI e os atributos variam por driver/kernel, e alguns
  arquivos de frequência são controles de configuração, não leitura de clock
  real.
- **Exigir Level Zero instalado ou linkar o loader:** rejeitada; host sem runtime
  Intel deve continuar funcionando normalmente.
- **Usar `zes_freq_state_t::request` como clock observado ou `CARD/PACKAGE` como
  potência GPU:** rejeitada por diferença de semântica/escopo.

## Consequências e critérios de revisão

A implementação atual espelha as declarações ABI do Level Zero 1.20.41, carrega
`libze_loader.so.1` (com fallback para `libze_loader.so`) em runtime e não cria
link obrigatório. `zeInit`, `zesInit`, `zeDriverGet`, `zeDeviceGet` e
`zeDeviceGetProperties` são requisitos; as funções de cada métrica são opcionais
e a ausência de símbolo deixa somente essa métrica ausente. A enumeração tem
limite de 256 handles por chamada para evitar alocação sem limite a partir de
contagens do driver. Antes de marcar `gpu_intel` como `supported`,
validar em hardware Intel real: iGPU e/ou dGPU alvo, driver/runtime e permissão;
identidade UUID/BDF; grupo `ALL` contra carga conhecida; memória local versus
SYSTEM e topologia multi-tile sem dupla contagem; unidades de clock, especialmente
memória; temperatura e escopo de potência; falhas parciais e runtime ausente. A
capability continua `unsupported` até esses critérios serem demonstrados. Fixtures,
fakes e CI não substituem homologação de hardware.

Referências oficiais consultadas:

- [Level Zero Sysman Programming Guide](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/PROG.html)
- [Level Zero Sysman API — Engine](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/engine.html)
- [Level Zero Sysman API — Device](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/device.html)
- [Level Zero Sysman API — Memory](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/memory.html)
- [Level Zero Sysman API — Frequency](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/frequency.html)
- [Level Zero Sysman API — Temperature](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/temperature.html)
- [Level Zero Sysman API — Power](https://oneapi-src.github.io/level-zero-spec/level-zero/latest/sysman/api/apis/power.html)
- [Level Zero headers and loader](https://github.com/oneapi-src/level-zero)
- [DRM client usage stats](https://docs.kernel.org/gpu/drm-usage-stats.html)
- [i915 DRM client usage stats](https://docs.kernel.org/gpu/i915.html)
- [Xe DRM client usage stats](https://docs.kernel.org/gpu/xe/xe-drm-usage-stats.html)
- [Xe GT frequency sysfs](https://docs.kernel.org/gpu/xe/xe_gt_freq.html)
- [Intel Compute Runtime issue documenting memory-frequency unit discrepancies](https://github.com/intel/compute-runtime/issues/932) (relato de implementação; não altera a unidade normativa da especificação)
