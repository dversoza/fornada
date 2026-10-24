# Fornada

Pão de Queijo Clicker reescrito em Rust e WebAssembly, hospedado na Cloudflare: https://fornada.versoza.dev

O protótipo original está em `reference/pao-de-queijo-clicker.html` e a especificação da reescrita em `PROMPT.md`.

## Arquitetura

A simulação roda no navegador com `requestAnimationFrame`. O servidor nunca faz tick por jogador: ele só guarda e valida saves.

A lógica do jogo fica no crate `game-core`, em Rust puro: catálogo, fórmulas, compras, cafezinho, ganho offline, formato de save e validação. O cliente usa o `game-core` compilado para WASM via `wasm-bindgen`. O servidor é um Worker escrito com `workers-rs`, também em WASM, então cliente e servidor executam exatamente o mesmo código.

O Worker atende só `/api/*` (`run_worker_first`). O resto sai direto do Static Assets, então a página abre sem esperar o Worker. Os saves ficam no D1 (binding `DB`), com o primário no leste dos EUA: nem D1 nem Durable Objects rodam na América do Sul.

```
game-core/   lógica compartilhada e testes
web/         wrapper wasm-bindgen e index.html (HTML/CSS do protótipo + JS fino)
worker/      Worker da API (crate api)
migrations/  esquema do D1
scripts/     build do cliente, testes da API e benchmark
tests/       testes de integração da API (node --test)
```

## Rodar local

Ferramentas: rustup (a versão vem de `rust-toolchain.toml`), `wasm-bindgen-cli` na mesma versão do `Cargo.lock`, binaryen (`wasm-opt`), `worker-build` e Node 24.

```bash
brew install rustup binaryen
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo install worker-build --version 0.8.7 --locked
npm ci
npx wrangler d1 migrations apply DB --local
npx wrangler dev
```

O `wrangler dev` roda o comando de build do `wrangler.jsonc`: `scripts/build-web.sh` gera `dist/` com nomes de arquivo com hash, e o `worker-build` compila o Worker.

Testes:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm test
```

`npm test` sobe o `wrangler dev` com um D1 local temporário e roda os testes de integração da API.

## Deploy

```bash
npx wrangler d1 migrations apply DB --remote
npx wrangler deploy
```

O token da Cloudflare precisa de D1 Edit, Workers Scripts Edit e, na zona versoza.dev, Workers Routes Edit. Use o wrangler: o `cf deploy` ignora o `wrangler.jsonc` deste projeto e gera uma configuração própria sem o Worker e sem o D1.

## Configuração

Não há segredos nem variáveis. O `wrangler.jsonc` define o domínio `fornada.versoza.dev` (`workers_dev` desligado), o binding `DB`, o Worker fixado perto do primário do D1 (`placement.region` `aws:us-east-1`) e Workers Logs. Os assets com hash recebem `Cache-Control: public, max-age=31536000, immutable` via `web/_headers`. O `index.html` revalida a cada acesso.

## API

| Rota | Função |
|---|---|
| `GET /api/save` | Devolve `{state, version, offline}`. Cria ou renova o cookie `__Host-pid`. O ganho offline usa o relógio do servidor e não é gravado. |
| `PUT /api/save` | Recebe `{state, version, schema_version}`. Versão 0 sem save existente importa o save local. Responde `{version, state, clamped}`. |
| `POST /api/reset` | Volta ao estado inicial e incrementa a versão. |

Erros: 401 sem sessão, 403 para requisição de outro site, 409 em versão desatualizada (com o estado atual no corpo), 413 acima de 8 KB, 415 sem JSON, 429 se o último save tem menos de 5 s.

O cliente salva a cada 30 s quando o estado muda, e também ao esconder ou fechar a aba, com `keepalive`. O botão Salvar agora, em Ajustes, força um save imediato. O cliente também mantém uma cópia em `localStorage`, então o jogo continua sem servidor e sincroniza depois.

## Dados

Cada jogador ocupa uma linha da tabela `saves`:

| Coluna | Tipo | Conteúdo |
|---|---|---|
| `player_id` | TEXT, chave primária | UUID do cookie `__Host-pid` |
| `state` | TEXT | save em JSON, formato v1 |
| `schema_version` | INTEGER | versão do formato do save (hoje 1) |
| `version` | INTEGER | contador da concorrência otimista |
| `updated_at` | INTEGER | último save aceito, em ms, relógio do servidor |
| `cafe_credit` | REAL | crédito de cafezinho do anti-trapaça |

O `state` usa o mesmo formato do save local do protótipo:

```json
{"bank":1274.79,"total":58323.79,"clicks":1254,"owned":{"vovo":32,"forno":22},"ups":["c1","vovo-0"],"buy":1}
```

Em produção, o `state` médio tem 65 bytes e o maior tem 189. Um save com todos os prédios e todas as melhorias tem 553 bytes. Com a chave e o índice, cada jogador ocupa de 150 a 700 bytes, então os 5 GB do plano Free comportam milhões de jogadores.

## Anti-trapaça

Cada PUT passa pelo `validate` do `game-core`. Prédios, melhorias, cliques e total nunca diminuem. As compras precisam bater com os custos, e melhorias bloqueadas são removidas. O crescimento do total fica limitado a `pps × (Δt + 10 s) + cliques × valor do clique + crédito × max(frenesi, café com prosa)`, com no máximo 40 cliques/s. O crédito de cafezinho fica no D1: ganha 1 a cada 30 s, até 2. Valores acima do limite são cortados e registrados no log, sem punição. A importação do save local só confere a consistência dos custos.

## Telemetria

O cliente envia a cada 5 minutos de jogo visível (e quando a aba sai de vista ou fecha, se passou de 60 s) uma amostra de desempenho para `POST /api/telemetry`: segundos cobertos, taxa do loop, tempo de JS por tick (média e p95 arredondado para a faixa 1/2/4/.../128 ms; não inclui estilo, layout e pintura), quadros travados (intervalo acima de 250 ms), fração parada, modo de efeitos, nível do pão, se o aparelho é de toque, `devicePixelRatio`, núcleos, memória e cliques por minuto.

A coleta é anônima: a requisição vai sem cookie (`credentials: 'omit'`) e a amostra não leva id, save nem horário. O Worker acrescenta só o país e uma família grosseira do navegador e grava uma linha no log. Os logs automáticos de cada requisição (`invocation_logs`) estão desligados, então nenhum IP, cabeçalho ou cidade é registrado. Contagem de requisições, erros e CPU continuam no painel de métricas do Worker.

Para ver os dados: painel da Cloudflare, Workers & Pages, fornada, Observability, filtro `event = perf`. O Workers Logs guarda os eventos por 3 dias.

## Medições

Feitas em 23/10/2026, logo após o deploy, a partir da máquina de desenvolvimento, com 50 jogadores sintéticos (`node scripts/bench.mjs https://fornada.versoza.dev 50`).

| Métrica | Valor |
|---|---|
| PUT (UPDATE) p50 / p95 | 334 ms / 672 ms |
| PUT (importação, INSERT) p50 / p95 | 331 ms / 744 ms |
| Startup do Worker (`wrangler deploy`) | 4 ms |
| Primeira requisição após o deploy | 369 ms |
| CPU por requisição p50 / p99 | 1,9 ms / 24 ms (105 requisições, 0 erros) |
| WASM do cliente | 159 KB, 75 KB com gzip |

Essas medições são de antes de fixar a região do Worker, quando ele rodava em São Paulo e cada PUT cruzava o continente duas vezes, uma por consulta ao D1. Com o Worker em Ashburn (`cf-placement: remote-IAD`), o Worker gasta 37 ms (p50) para validar e fazer as duas consultas. O PUT medido do Brasil continua perto de 300 ms (p50 de 348 ms em 6 amostras), porque a maior parte é a rede entre o Brasil e os EUA. Nem o D1 nem os Durable Objects rodam na América do Sul. Como o save roda em segundo plano, o jogador não sente essa latência. O p99 de CPU vem da primeira requisição de cada isolate, quando o WASM compila.

## Custo

No plano Free o custo é zero. Os limites diários são 100 mil requisições ao Worker e 100 mil linhas gravadas no D1. Cada PUT grava uma linha, e os estáticos não contam. Com um save a cada 30 s, isso dá cerca de 830 horas de jogo por dia somando todos os jogadores. Para uns 1.000 jogadores ativos, o plano Workers Paid (US$ 5/mês) cobre o uso dentro da franquia.

## Limites conhecidos

Quantidades são `f64`. Inteiros são exatos até 2^53 (cerca de 9 quatrilhões). Acima disso, incrementos pequenos se perdem, e perto de 1,8e308 o valor vira infinito.

Acima de cerca de 4,4e14, o custo pode diferir em 1 do protótipo: o `pow` do WASM vem do musl e o do navegador é o do V8. Cliente e servidor sempre concordam entre si.

O buff de produção ×7 só vale até expirar, mesmo num frame longo com a aba oculta. No protótipo ele valia pelo tempo todo em que a aba ficava escondida.

A CPU do Worker passa do limite de 10 ms do plano Free nas primeiras requisições de cada isolate novo: as cerca de 10 primeiras custam de 7 a 19 ms, e as seguintes de 1 a 2 ms. Com pouco tráfego os isolates esfriam, então nas 24 h medidas em 24/10/2026 o p90 foi 12 ms e o p99 22 ms, com 0 erros em 819 requisições. Ajustes de compilação (`opt-level`, `wasm-opt`, `panic=abort`) mudaram menos de 1%. Se aparecerem erros de CPU excedida (status `exceededCpu` no GraphQL `workersInvocationsAdaptive`), as saídas são o Workers Paid (limite de 30 s) ou mover o roteamento para JS e deixar o WASM só na validação.

Qualquer pessoa pode criar saves em massa e esgotar a cota diária de gravações do D1. O jogo segue funcionando offline nesse caso. A correção é uma regra de rate limit no WAF da zona, que exige a permissão Zone WAF Edit.

A folga de 10 s por save permite que quem envia um PUT a cada 5 s ganhe até 3× a produção.

## Diferenças em relação ao protótipo

Os prédios ainda não revelados não aparecem mais como caixas vazias na loja. No protótipo, o `.item{display:flex}` anulava o atributo `hidden`.

O pão evolui com a produção: a aura começa em 100/s, o brilho em 10 mil/s, a borda dourada em 1 milhão/s e os raios em 1 bilhão/s. Ícones dos prédios comprados giram em volta dele. Uma vitrine abaixo mostra cada prédio com a quantidade, e cada compra voa da loja até o pão. As animações contínuas respeitam `prefers-reduced-motion`, pausam após 20 s sem toque e podem ser desligadas em Ajustes (Efeitos animados, guardado em `localStorage` como `pdq-fx`). Se o sistema pede movimento reduzido, essa preferência vence o botão.

O loop do jogo roda a cerca de 15 Hz (`setTimeout` + `requestAnimationFrame`), com o tempo real decorrido, então a simulação continua exata. Medido em Chrome headless com CPU 4× mais lenta, num save avançado: CPU de 11,8% para 5,2%, 3,75% parado após 20 s e 3,2% com os efeitos desligados.
