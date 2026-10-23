# Pão de Queijo Clicker: reescrita em Rust na Cloudflare

## Objetivo
Reescrever o protótipo em `reference/pao-de-queijo-clicker.html` como app em Rust, hospedado na Cloudflare, com o progresso do jogador salvo no servidor de forma durável.

O protótipo é a especificação: fórmulas, valores, textos, visual e comportamento. Preserve o balanceamento:
- Custo de compra: `ceil(base * 1.15^owned * (1.15^n - 1) / 0.15)`
- Melhorias de clique (×2), de "mão de vó" (+1% da produção por clique) e por prédio (×2, nos marcos 1/10/25)
- Cafezinho: 10% cliques ×77 por 13s, 45% produção ×7 por 30s, 45% ganho de `min(bank*0.15, pps_base*900) + 13`
- Ganho offline: 50% da produção, limitado a 12h

## Passo 0: decidir a arquitetura antes de codar
Consulte a documentação atual da Cloudflare (Containers, Workers, Durable Objects, D1) e me apresente uma comparação curta entre:
- **A) Cloudflare Containers**: servidor Rust (axum + tokio) em imagem Docker, com um Worker/Durable Object na frente.
- **B) workers-rs**: Rust compilado para WASM rodando direto no Worker, com D1 ou SQLite de Durable Object.

Compare cold start, latência, custo mensal estimado para ~1.000 jogadores ativos, complexidade e como cada uma acessa armazenamento durável (disco do container não conta como longo prazo). Minha preferência inicial é A; espere minha confirmação antes de seguir.

## Arquitetura alvo
- **A simulação roda no cliente** (requestAnimationFrame). O servidor nunca faz tick por jogador.
- **Crate `game-core`** em Rust puro: estado, fórmulas, compras, aplicação de tempo decorrido e validação de saves. Compila para WASM (cliente) e nativo (servidor), garantindo a mesma lógica nos dois lados.
- **Frontend**: reaproveite HTML/CSS do protótipo; JS/TS fino cuida do DOM e chama o WASM. Não use framework de UI em Rust (Leptos/Yew) sem justificar o ganho frente ao tamanho do bundle.
- **Estáticos servidos pelo Worker** (Static Assets), para a página abrir sem esperar o backend acordar.
- **Persistência durável**: D1 ou SQLite de Durable Object (ou Postgres via Hyperdrive, se justificar). Explique como o backend escolhido chega até ela.

## Sessão e dados
- Jogador anônimo: UUID gerado no servidor, cookie `HttpOnly; Secure; SameSite=Lax`, validade de 1 ano.
- Tabela `saves(player_id PK, state, schema_version, version, updated_at)`. Versione o formato do save e escreva migração.
- Endpoints: `GET /api/save`, `PUT /api/save` (concorrência otimista via `version`, para duas abas abertas não se sobrescreverem), `POST /api/reset`.
- Ganho offline calculado com o `updated_at` do servidor, nunca com o relógio do cliente.
- Se existir save local `pdq-clicker-v1` e o servidor estiver vazio, importe uma vez.

## Anti-trapaça leve
Valide cada `PUT` com o `game-core`: o crescimento de `total` desde o último save aceito não pode passar de `pps_max * Δt * buff_max + cliques_plausíveis` (ex.: 20 cliques/s). Compras precisam bater com os custos. Em caso de inconsistência, faça clamp para o máximo plausível e registre no log; sem punição.

## Performance
- Cliente: atualizar o DOM só quando o valor muda (como no protótipo), limitar partículas, evitar layout thrash.
- `f64` para quantidades; documente o limite.
- Rede: salvar com debounce a cada 15–30s e no `visibilitychange`/`pagehide` via `fetch(..., {keepalive: true})`. Payload JSON compacto, alvo < 2 KB.
- WASM com `wasm-opt -Oz`, alvo < 150 KB gzip. Estáticos com hash no nome e cache longo.
- Se A: imagem multi-stage, binário estático (musl), alvo < 20 MB; configure o tempo de sleep do container.
- Meça p50/p95 do `PUT /api/save` e o cold start; registre no README.

## Qualidade
- Testes unitários do `game-core` com casos derivados do protótipo (custos, multiplicadores, offline, validação).
- Testes de integração da API.
- `cargo fmt`, `cargo clippy -- -D warnings`.
- README: rodar local, deploy, variáveis/bindings, custo estimado.

## Etapas (pare ao fim de cada uma com um resumo curto)
1. Passo 0 (decisão)
2. `game-core` + testes
3. Frontend usando o WASM, com paridade visual com o protótipo
4. Backend + persistência
5. Deploy + README

## Fora do escopo por agora
Login, ranking, multiplayer, mudanças de balanceamento sem me consultar, segredos no repositório.
