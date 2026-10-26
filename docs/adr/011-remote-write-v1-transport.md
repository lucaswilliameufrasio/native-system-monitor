# ADR-011: primeira fatia de transporte Prometheus Remote Write v1

- **Status:** Aceito para transporte sem autenticação da primeira fatia de M5
- **Data:** 2026-10-26
- **Referência:** plano mestre, seções 9.1–9.3 e incremento M5.

## Contexto

O pull OpenMetrics local já está disponível, mas M5 também requer envio remoto
Remote Write v1 sem bloquear a coleta. O protocolo fixa protobuf `WriteRequest`,
compressão Snappy block e headers HTTP; o plano exige HTTPS, limites explícitos,
retries transitórios e credenciais guardadas fora do TOML.

## Decisão

- Reutilizar o renderer de famílias atual para produzir amostras Remote Write v1,
  ordenando labels e incluindo `__name__`; comprimir protobuf com Snappy block.
- Usar cliente HTTP assíncrono com validação TLS padrão, timeout finito e
  redirects desabilitados. Aceitar HTTPS em destinos configurados; permitir HTTP
  somente para `localhost`/loopback em ensaios locais. Rejeitar userinfo, query e
  fragmento na URL para impedir tokens em texto claro ou parâmetros sensíveis.
- Desabilitar Remote Write por padrão. Criar lotes a partir do snapshot existente,
  sem iniciar coleta extra; manter fila FIFO volátil limitada por bytes e número
  de lotes. Quando cheia, rejeitar o lote novo para preservar a ordem já aceita.
- Repetir falhas de transporte, 408, 425, 429 e 5xx com backoff exponencial
  limitado e jitter; respeitar `Retry-After` em segundos até um teto. Descartar
  respostas 4xx permanentes sem retry cego. Um timeout pode deixar o resultado
  remoto incerto; repetir o mesmo timestamp/amostra pode duplicar requisições,
  então não se promete exactly-once.
- Manter contadores atômicos em memória compartilhados entre o worker e o
  endpoint pull: habilitado, lotes/bytes pendentes, sucessos, tentativas
  transitórias, rejeições permanentes, descartes por fila cheia e timestamp do
  último sucesso. Não adicionar labels com endpoint, host ou credenciais. Esses
  valores reiniciam junto com o agente e não substituem spool durável.
- Não configurar autenticação nesta fatia: credential stores nativos, rotação,
  revogação e credential_ref permanecem pendentes. A configuração aceita apenas
  receptores que não exijam autenticação até essa integração existir.

## Alternativas

- Usar HTTP sem TLS para qualquer destino: rejeitado por expor métricas e
  contrariar a fronteira de segurança estabelecida no plano.
- Seguir redirects: rejeitado para não transferir futuras credenciais ou amostras
  a outra origem sem política explícita.
- Persistir imediatamente a fila em disco: adiado; exige formato versionado,
  limites de disco, permissões, recuperação e política de limpeza. A fila em
  memória perde lotes no encerramento, com aviso local.
- Prometer entrega exactly-once: rejeitado; Remote Write pode receber uma
  repetição após resposta perdida e não há transação compartilhada com o
  receptor.

## Consequências e revisão

Encoding, headers, URL, fila e classificação têm testes locais com receptor
HTTP falso. O laboratório `scripts/test-victoriametrics-remote-write.sh` foi
executado contra VictoriaMetrics v1.154.0 em container isolado; uma série CPU foi
indexada e lida pela API de séries. `scripts/test-prometheus-remote-write.sh`
também foi executado contra Prometheus v3.15.1 com
`--web.enable-remote-write-receiver`: a série CPU foi consultada pela API
Prometheus após Remote Write v1, e o container/rede do ensaio foram removidos.
Essas evidências validam receptores sem autenticação, não todos os destinos nem
configurações TLS reais. Permanecem credential stores por OS, rotação/revogação,
spool opcional e validação de destinos autenticados; estes continuam indisponíveis
até a integração de credenciais existir.
