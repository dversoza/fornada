// Usage: node scripts/bench.mjs <base_url> <n>
// The Worker refuses a second PUT within 5 s of the last one, so one sequential run cannot
// time updates. Round 1 imports n new players (INSERT path); after 5.1 s round 2 updates
// each of them (UPDATE path). Both rounds are sequential.
const [base, nArg] = process.argv.slice(2);
const n = Number(nArg);
if (!base || !(n > 0)) { console.error('usage: node scripts/bench.mjs <base_url> <n>'); process.exit(2); }

const timed = async (fn) => { const t = performance.now(); const res = await fn(); return [res, performance.now() - t]; };
const put = (cookie, state, version) => fetch(`${base}/api/save`, {
  method: 'PUT',
  headers: { 'content-type': 'application/json', cookie },
  body: JSON.stringify({ state, version, schema_version: 1 }),
});
const stats = (label, times) => {
  const t = [...times].sort((a, b) => a - b);
  const q = (p) => t[Math.min(t.length - 1, Math.floor(p * t.length))].toFixed(1);
  console.log(`${label} n=${t.length} p50=${q(0.5)} ms p95=${q(0.95)} ms max=${t.at(-1).toFixed(1)} ms`);
};

const players = [];
let firstMs;
for (let i = 0; i < n; i++) {
  const [res, ms] = await timed(() => fetch(`${base}/api/save`));
  if (i === 0) firstMs = ms;
  await res.arrayBuffer();
  players.push({ cookie: res.headers.getSetCookie()[0].split(';')[0], state: { bank: 0, total: 0, clicks: 0, owned: {}, ups: [], buy: 1 }, version: 0 });
}
console.log(`first request (GET, new cookie): ${firstMs.toFixed(1)} ms`);

const round = async (label) => {
  const times = [];
  for (const p of players) {
    p.state = { ...p.state, bank: p.state.bank + 1, total: p.state.total + 1, clicks: p.state.clicks + 1 };
    const [res, ms] = await timed(() => put(p.cookie, p.state, p.version));
    if (!res.ok) { console.error(`${label} PUT failed: ${res.status} ${await res.text()}`); process.exit(1); }
    const body = await res.json();
    if (body.clamped) { console.error(`${label} PUT was clamped`); process.exit(1); }
    p.version = body.version;
    times.push(ms);
  }
  stats(label, times);
};
await round('PUT import (INSERT)');
await new Promise((r) => setTimeout(r, 5100));
await round('PUT update (UPDATE)');
