import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const BASE = process.env.BASE_URL;
const PERSIST = process.env.PERSIST_DIR;
const LOG_FILE = process.env.LOG_FILE;
const COOKIE = '__Host-pid';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function d1(sql) {
  const out = execFileSync('npx', ['wrangler', 'd1', 'execute', 'DB', '--local', '--persist-to', PERSIST, '--json', '--command', sql], { encoding: 'utf8' });
  return JSON.parse(out)[0].results;
}

const cost = (base, owned, n) => Math.ceil(base * 1.15 ** owned * (1.15 ** n - 1) / 0.15);

/** 10 vovo (1/s) + 10 forno (10/s), paid exactly: base pps 11. */
const SPENT = cost(15, 0, 10) + cost(100, 0, 10);
const honest = { bank: 100000 - SPENT, total: 100000, clicks: 0, owned: { vovo: 10, forno: 10 }, ups: [], buy: 1 };
const BASE_PPS = 11;

async function player() {
  const res = await fetch(`${BASE}/api/save`);
  const cookie = res.headers.getSetCookie()[0].split(';')[0];
  await res.arrayBuffer();
  const pid = cookie.slice(COOKIE.length + 1);
  return {
    pid,
    call: (method, path, body, headers = {}) => fetch(`${BASE}${path}`, {
      method,
      headers: { cookie, 'content-type': 'application/json', ...headers },
      body: body === undefined ? undefined : typeof body === 'string' ? body : JSON.stringify(body),
    }),
    get: (p = '/api/save') => fetch(`${BASE}${p}`, { headers: { cookie } }),
    put: (state, version) => fetch(`${BASE}/api/save`, {
      method: 'PUT', headers: { cookie, 'content-type': 'application/json' }, body: JSON.stringify({ state, version, schema_version: 1 }),
    }),
    /** Moves updated_at back so the 5 s PUT floor does not apply. */
    age: (ms = 60000) => d1(`UPDATE saves SET updated_at = updated_at - ${ms} WHERE player_id = '${pid}'`),
  };
}

test('cookie is issued and refreshed with the right attributes and a malformed one is replaced', async () => {
  const res = await fetch(`${BASE}/api/save`);
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('cache-control'), 'no-store');
  assert.match(res.headers.get('content-type'), /^application\/json/);
  assert.deepEqual(await res.json(), { state: null, version: 0, offline: 0 });
  const cookies = res.headers.getSetCookie();
  assert.equal(cookies.length, 1);
  const [pair, ...attrs] = cookies[0].split('; ');
  assert.match(pair.slice(COOKIE.length + 1), UUID);
  assert.deepEqual(attrs, ['HttpOnly', 'Secure', 'SameSite=Lax', 'Path=/', 'Max-Age=31536000']);

  const again = await fetch(`${BASE}/api/save`, { headers: { cookie: pair } });
  assert.deepEqual(again.headers.getSetCookie(), cookies, 'valid cookie is re-sent for a sliding expiry');
  assert.deepEqual(await again.json(), { state: null, version: 0, offline: 0 });

  for (const bad of [`${COOKIE}=not-a-uuid`, `${COOKIE}=${pair.slice(COOKIE.length + 1)}x`, `${COOKIE}=`, `pid=${pair.slice(COOKIE.length + 1)}`, 'other=1']) {
    const r = await fetch(`${BASE}/api/save`, { headers: { cookie: bad } });
    const set = r.headers.getSetCookie();
    assert.equal(set.length, 1, bad);
    assert.match(set[0].split(';')[0].slice(COOKIE.length + 1), UUID);
    assert.notEqual(set[0].split(';')[0], pair);
    await r.arrayBuffer();
  }
});

test('import once, versions increment, stale tab gets 409 with the current state', async () => {
  const p = await player();
  let res = await p.put(honest, 0);
  assert.equal(res.status, 200);
  let body = await res.json();
  assert.equal(body.version, 1);
  assert.equal(body.clamped, false);
  assert.deepEqual(body.state, honest);

  res = await p.put({ ...honest, bank: 0, total: 5 }, 0);
  assert.equal(res.status, 409);
  assert.deepEqual(await res.json(), { state: honest, version: 1 });

  const next = { ...honest, bank: honest.bank + 1, total: honest.total + 1, clicks: 1 };
  p.age();
  res = await p.put(next, 1);
  assert.equal(res.status, 200);
  body = await res.json();
  assert.deepEqual([body.version, body.clamped, body.state], [2, false, next]);

  // second tab still on version 1
  res = await p.put({ ...next, clicks: 2, bank: next.bank + 1, total: next.total + 1 }, 1);
  assert.equal(res.status, 409);
  assert.deepEqual(await res.json(), { state: next, version: 2 });

  res = await p.get();
  body = await res.json();
  assert.equal(body.version, 2);
  assert.equal(body.offline, 0);
  assert.deepEqual(body.state, next);
});

test('offline gain uses the server clock and is not persisted', async () => {
  const p = await player();
  assert.equal((await p.put(honest, 0)).status, 200);
  const moved = Date.now() - 7200 * 1000;
  d1(`UPDATE saves SET updated_at = ${moved} WHERE player_id = '${p.pid}'`);
  const before = d1(`SELECT * FROM saves WHERE player_id = '${p.pid}'`)[0];

  // The server clock cannot be frozen, so check the gain against the elapsed window it saw.
  const t0 = Date.now();
  const body = await (await p.get()).json();
  const t1 = Date.now();
  const elapsed = body.offline / (BASE_PPS * 0.5);
  assert.ok(elapsed * 1000 >= t0 - moved - 1 && elapsed * 1000 <= t1 - moved + 1, `${elapsed} s`);
  assert.ok(elapsed >= 7200 && elapsed < 7260, `${elapsed} s`);
  assert.equal(body.version, 1);
  assert.equal(body.state.bank, honest.bank + body.offline);
  assert.equal(body.state.total, honest.total + body.offline);

  assert.deepEqual(d1(`SELECT * FROM saves WHERE player_id = '${p.pid}'`)[0], before);

  // Past the 12 h cap the gain is exact: base pps * 43200 s * 0.5.
  d1(`UPDATE saves SET updated_at = ${Date.now() - 13 * 3600 * 1000} WHERE player_id = '${p.pid}'`);
  const capped = await (await p.get()).json();
  assert.equal(capped.offline, BASE_PPS * 43200 * 0.5);
  assert.equal(capped.state.total, honest.total + BASE_PPS * 43200 * 0.5);
});

test('an inflated total is clamped to the exact bound and reported', async () => {
  const p = await player();
  // No production, so the time window adds nothing and the bound is the credit alone.
  const idle = { bank: 1000, total: 1000, clicks: 0, owned: {}, ups: [], buy: 1 };
  assert.equal((await p.put(idle, 0)).status, 200);
  // Credit 2 stays at the cap whatever the elapsed time, so the bound is exact:
  // 2 cafe frenzies of 76 * 13 * 40 * click value 1.
  d1(`UPDATE saves SET cafe_credit = 2.0 WHERE player_id = '${p.pid}'`);
  p.age();

  const res = await p.put({ ...idle, bank: 1e12, total: 1e12 }, 1);
  assert.equal(res.status, 200);
  const body = await res.json();
  const allowed = 2 * 39520;
  assert.equal(body.clamped, true);
  assert.equal(body.version, 2);
  assert.equal(body.state.total, 1000 + allowed);
  assert.equal(body.state.bank, 1000 + allowed);

  const row = d1(`SELECT state, cafe_credit FROM saves WHERE player_id = '${p.pid}'`)[0];
  assert.deepEqual(JSON.parse(row.state), body.state);
  assert.ok(Math.abs(row.cafe_credit) < 1e-9, row.cafe_credit);
});

test('PUTs closer than 5 s get 429 and write nothing', async () => {
  const p = await player();
  assert.equal((await p.put(honest, 0)).status, 200);
  const next = { ...honest, bank: honest.bank + 1, total: honest.total + 1, clicks: 1 };
  const before = d1(`SELECT * FROM saves WHERE player_id = '${p.pid}'`)[0];
  const res = await p.put(next, 1);
  assert.equal(res.status, 429);
  assert.equal(res.headers.get('retry-after'), '5');
  assert.equal(res.headers.get('cache-control'), 'no-store');
  assert.deepEqual(d1(`SELECT * FROM saves WHERE player_id = '${p.pid}'`)[0], before);

  p.age(6000);
  assert.equal((await p.put(next, 1)).status, 200);
});

test('cross-origin and non-JSON writes are refused', async () => {
  const p = await player();
  const body = JSON.stringify({ state: honest, version: 0, schema_version: 1 });
  for (const path of ['/api/save', '/api/reset']) {
    const method = path === '/api/save' ? 'PUT' : 'POST';
    for (const site of ['cross-site', 'same-site', 'none']) {
      const res = await p.call(method, path, body, { 'sec-fetch-site': site });
      assert.equal(res.status, 403, `${method} ${path} ${site}`);
      await res.arrayBuffer();
    }
    for (const type of ['text/plain', 'application/x-www-form-urlencoded', 'multipart/form-data; boundary=x']) {
      const res = await p.call(method, path, body, { 'content-type': type });
      assert.equal(res.status, 415, `${method} ${path} ${type}`);
      await res.arrayBuffer();
    }
  }
  assert.deepEqual(d1(`SELECT * FROM saves WHERE player_id = '${p.pid}'`), [], 'refused requests wrote nothing');

  const ok = await p.call('PUT', '/api/save', body, { 'sec-fetch-site': 'same-origin', 'content-type': 'application/json; charset=utf-8' });
  assert.equal(ok.status, 200);
});

test('reset keeps the row, so the import path stays closed', async () => {
  const fresh = await player();
  let res = await fresh.call('POST', '/api/reset');
  assert.equal(res.status, 200);
  const first = await res.json();
  assert.equal(first.version, 1);
  assert.deepEqual(first.state, { bank: 0, total: 0, clicks: 0, owned: {}, ups: [], buy: 1 });

  const p = await player();
  assert.equal((await p.put(honest, 0)).status, 200);
  res = await p.call('POST', '/api/reset');
  const body = await res.json();
  assert.equal(body.version, 2);
  assert.deepEqual(body.state, first.state);

  res = await p.put(honest, 0);
  assert.equal(res.status, 409);
  assert.deepEqual(await res.json(), { state: first.state, version: 2 });

  const row = d1(`SELECT version, cafe_credit FROM saves WHERE player_id = '${p.pid}'`)[0];
  assert.deepEqual(row, { version: 2, cafe_credit: 1 });
});

test('error statuses', async () => {
  const p = await player();
  const noCookie = { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ state: honest, version: 0, schema_version: 1 }) };
  let res = await fetch(`${BASE}/api/save`, noCookie);
  assert.equal(res.status, 401);
  assert.equal(res.headers.get('cache-control'), 'no-store');
  assert.equal((await fetch(`${BASE}/api/reset`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' })).status, 401);

  for (const bad of ['{not json', '{}', '{"state":{"bank":"x"},"version":0}', '{"state":{},"version":-1}', '{"state":{}}']) {
    res = await p.call('PUT', '/api/save', bad);
    assert.equal(res.status, 400, bad);
    await res.arrayBuffer();
  }
  res = await p.call('PUT', '/api/save', { state: honest, version: 0 });
  assert.equal(res.status, 400, 'schema_version is required');
  await res.arrayBuffer();
  for (const schema_version of [0, 2]) {
    res = await p.call('PUT', '/api/save', { state: honest, version: 0, schema_version });
    assert.equal(res.status, 400);
    assert.deepEqual(await res.json(), { error: 'unsupported schema' });
  }
  const huge = JSON.stringify({ state: { ...honest, ups: Array(2000).fill('c1') }, version: 0, schema_version: 1 });
  assert.ok(huge.length > 8 * 1024);
  assert.equal((await p.call('PUT', '/api/save', huge)).status, 413);

  res = await p.get('/api/nope');
  assert.equal(res.status, 404);
  assert.match(res.headers.get('content-type'), /json/);
  assert.equal((await p.call('DELETE', '/api/save')).status, 405);
  res = await p.get('/api/reset');
  assert.equal(res.status, 405);
  assert.equal(res.headers.get('allow'), 'POST');
  assert.equal((await p.call('POST', '/api/save', {})).status, 405);
});

const SAMPLE = { secs: 300, hz: 14.9, render_ms_avg: 2.5, render_ms_p95: 4, jank: 1, idle_frac: 0.2, fx: 'on', tier: 2, mobile: true, dpr: 3, cores: 6, mem: null, clicks_per_min: 40 };
const telemetry = (body, headers = {}) => fetch(`${BASE}/api/telemetry`, {
  method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body: typeof body === 'string' ? body : JSON.stringify(body),
});

const perfLine = async (match) => {
  const deadline = Date.now() + 5000;
  let line;
  while (!line && Date.now() < deadline) {
    line = readFileSync(LOG_FILE, 'utf8').split('\n').find((l) => l.includes('"event":"perf"') && match(l));
    if (!line) await new Promise((r) => setTimeout(r, 100));
  }
  assert.ok(line, 'perf line reached the log');
  return line;
};

test('telemetry accepts a valid sample, logs it without the pid and refuses bad requests', async () => {
  const p = await player();
  const ua = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Version/17.0 Mobile/15E148 Safari/604.1';
  let res = await telemetry({ ...SAMPLE, extra: 'ignored' }, { cookie: `${COOKIE}=${p.pid}`, 'user-agent': ua, 'sec-fetch-site': 'same-origin' });
  assert.equal(res.status, 204);
  assert.equal(await res.text(), '');

  const line = await perfLine(() => true);
  assert.ok(!line.includes(p.pid), 'log line has no pid');
  const logged = JSON.parse(line.slice(line.indexOf('{')));
  assert.deepEqual([logged.event, logged.v, logged.ua, logged.tier, logged.secs], ['perf', 1, 'ios-safari', 2, 300]);
  assert.equal('extra' in logged, false);

  res = await telemetry({ ...SAMPLE, secs: -5, idle_frac: 3, tier: 9, hz: 1000, mem: 9999, clicks_per_min: 123.5 });
  assert.equal(res.status, 204);
  const out = await perfLine((l) => l.includes('"clicks_per_min":123.5'));
  const clamped = JSON.parse(out.slice(out.indexOf('{')));
  assert.deepEqual([clamped.secs, clamped.idle_frac, clamped.tier, clamped.hz, clamped.mem], [0, 1, 5, 240, 512]);

  for (const bad of ['{not json', '{}', JSON.stringify({ ...SAMPLE, fx: 'loud' }), JSON.stringify({ ...SAMPLE, hz: '14' }), '{"secs":1e999}']) {
    res = await telemetry(bad);
    assert.equal(res.status, 400, bad);
    await res.arrayBuffer();
  }
  res = await telemetry(SAMPLE, { 'sec-fetch-site': 'cross-site' });
  assert.equal(res.status, 403);
  res = await telemetry(SAMPLE, { 'content-type': 'text/plain' });
  assert.equal(res.status, 415);
  res = await telemetry(JSON.stringify({ ...SAMPLE, pad: 'x'.repeat(1100) }));
  assert.equal(res.status, 413);
  res = await fetch(`${BASE}/api/telemetry`);
  assert.equal(res.status, 405);
  assert.equal(res.headers.get('allow'), 'POST');
});

test('static assets are served without the worker and assets are immutable', async () => {
  const res = await fetch(`${BASE}/`);
  assert.equal(res.status, 200);
  assert.match(res.headers.get('content-type'), /text\/html/);
  // The Worker sets no-store on everything it answers, so this proves the Worker was skipped.
  assert.notEqual(res.headers.get('cache-control'), 'no-store');
  assert.equal(res.headers.get('set-cookie'), null);
  const html = await res.text();
  assert.match(html, /Pão de Queijo/);
  const assets = [...html.matchAll(/["'](\/assets\/[^"']+)["']/g)].map((m) => m[1]);
  assert.ok(assets.length >= 2, assets.join());
  for (const a of assets) {
    const r = await fetch(`${BASE}${a}`);
    assert.equal(r.status, 200, a);
    assert.equal(r.headers.get('cache-control'), 'public, max-age=31536000, immutable', a);
    assert.equal(r.headers.get('set-cookie'), null, a);
    await r.arrayBuffer();
  }
});
