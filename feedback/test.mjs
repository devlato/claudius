// node feedback/test.mjs: the endpoint keeps the feedback claudius sends, and nothing else
import worker from './src/index.js';
const kept = [];
let allowed = true;
const env = {
  FEEDBACK: { prepare: (sql) => ({ bind: (...row) => ({ run: async () => kept.push({ sql, row }) }) }) },
  LIMIT: { limit: async () => ({ success: allowed }) },
};
const send = (body, { path = '/v1/feedback', method = 'POST', origin } = {}) =>
  worker.fetch(
    new Request('https://x.workers.dev' + path, {
      method,
      headers: origin ? { origin } : {},
      body: method === 'POST' ? (typeof body === 'string' ? body : JSON.stringify(body)) : undefined,
    }),
    env,
  );
const base = { v: 1, text: 'The radio should mix when I skip.', where: 'studio' };
const cases = [
  ['a message', base, 204],
  ['with where to answer, the version, the system and the genre', { ...base, contact: 'me@example.com', version: '0.14.0', os: 'macos', song: 'trip_hop' }, 204],
  ['from the demo', { ...base, where: 'demo', os: 'web' }, 204],
  ['from an assistant', { ...base, where: 'assistant' }, 204],
  ['a fusion', { ...base, song: 'trip_hop+idm' }, 204],
  ['a fusion of four', { ...base, song: 'trip_hop+neo_soul+dub_techno+idm' }, 204],
  ['a song name too long', { ...base, song: Array(8).fill('drum_and_bass_x').join('+').repeat(2) }, 400],
  ['nothing written', { ...base, text: '   ' }, 400],
  ['too much written', { ...base, text: 'x'.repeat(4001) }, 400],
  ['from somewhere else', { ...base, where: 'email' }, 400],
  ['an extra field', { ...base, ip: '1.2.3.4' }, 400],
  ['a long address', { ...base, contact: 'x'.repeat(201) }, 400],
  ['a version that is not one', { ...base, version: 'latest' }, 400],
  ['a song that is not a genre', { ...base, song: '../../etc' }, 400],
  ['another version of the message', { ...base, v: 2 }, 400],
  ['a long body', 'x'.repeat(7000), 413],
  ['not JSON', '{', 400],
];
let ok = true;
for (const [name, body, want] of cases) {
  const r = await send(body);
  if (r.status !== want) {
    ok = false;
    console.log(`FAIL ${name}: ${r.status}, wanted ${want}`);
  }
}
const n = cases.filter((c) => c[2] === 204).length;
if (kept.length !== n) {
  ok = false;
  console.log(`FAIL kept ${kept.length}, wanted ${n}`);
}
const first = kept[1].row;
if (!/^\d{4}-\d{2}-\d{2}$/.test(first[0]) || first[1] !== base.text || first[2] !== 'me@example.com' || first[5] !== 'studio' || first[6] !== 'trip_hop') {
  ok = false;
  console.log('FAIL what was kept', first);
}
// a browser's page of the site may send it, another page may not read the answer
const site = await send(base, { origin: 'https://claudius.devlato.com' });
const other = await send(base, { origin: 'https://example.com' });
if (site.headers.get('access-control-allow-origin') !== 'https://claudius.devlato.com' || other.headers.get('access-control-allow-origin')) {
  ok = false;
  console.log('FAIL the origins');
}
const pre = await send(null, { method: 'OPTIONS', origin: 'https://claudius.devlato.com' });
if (pre.status !== 204 || pre.headers.get('access-control-allow-methods') !== 'POST') {
  ok = false;
  console.log('FAIL the preflight');
}
// too many at once, and anything but a POST to its path
allowed = false;
if ((await send(base)).status !== 429) { ok = false; console.log('FAIL the limit'); }
allowed = true;
if ((await send(null, { method: 'GET' })).status !== 405) { ok = false; console.log('FAIL a GET'); }
if ((await send(base, { path: '/' })).status !== 404) { ok = false; console.log('FAIL another path'); }
console.log(ok ? 'all passed' : 'some failed');
process.exit(ok ? 0 : 1);
