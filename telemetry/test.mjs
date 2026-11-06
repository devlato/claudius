// node telemetry/test.mjs: the endpoint takes what claudius sends and refuses anything else
import worker from './src/index.js';
const written = [];
const env = { EVENTS: { writeDataPoint: (p) => written.push(p) } };
const post = (body, path = '/v1/event', method = 'POST') =>
  worker.fetch(new Request('https://x.workers.dev' + path, { method, body: method === 'POST' ? (typeof body === 'string' ? body : JSON.stringify(body)) : undefined }), env);
const base = { v: 1, event: 'started', command: 'studio', version: '0.2.0', os: 'macos', arch: 'aarch64', detail: '' };
const two = { ...base, v: 2, version: '0.5.0', value: 1 };
const cases = [
  ['started', base, 204],
  ['started, version 2', two, 204],
  ['a render speed', { ...two, event: 'perf', detail: 'render', value: 38 }, 204],
  ['the ears\' speed', { ...two, event: 'perf', command: 'serve', detail: 'ears', value: 0.42 }, 204],
  ['a speed of nothing', { ...two, event: 'perf', detail: 'render', value: 0 }, 400],
  ['a speed of something else', { ...two, event: 'perf', detail: 'mixing', value: 3 }, 400],
  ['a speed in text', { ...two, event: 'perf', detail: 'render', value: '38' }, 400],
  ['a speed in version 1', { ...base, event: 'perf', detail: 'render' }, 400],
  ['a count of more than one', { ...two, value: 5 }, 400],
  ['version 2 without a value', { ...base, v: 2 }, 400],
  ['setup failed on a pack', { ...base, event: 'setup', command: 'setup', detail: 'packs:vcsl' }, 204],
  ['a panic', { ...base, event: 'panic', detail: 'claudius/src/render.rs:123' }, 204],
  ['a pre-release', { ...base, version: '0.3.0-rc.1' }, 204],
  ['an extra field', { ...base, user: 'x' }, 400],
  ['a path in a panic', { ...base, event: 'panic', detail: '/Users/me/x/claudius/src/a.rs:1' }, 400],
  ['an unknown command', { ...base, command: 'rm' }, 400],
  ['a detail on started', { ...base, detail: 'hi' }, 400],
  ['a long body', 'x'.repeat(600), 413],
  ['not JSON', '{', 400],
];
let ok = true;
for (const [name, body, want] of cases) {
  const r = await post(body);
  const pass = r.status === want;
  ok &&= pass;
  console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}: ${r.status}`);
}
const g = await post(null, '/v1/event', 'GET');
console.log(`${g.status === 405 ? 'ok  ' : 'FAIL'} GET: ${g.status}`);
const other = await post(base, '/');
console.log(`${other.status === 404 ? 'ok  ' : 'FAIL'} another path: ${other.status}`);
const unbound = await worker.fetch(new Request('https://x.workers.dev/v1/event', { method: 'POST', body: JSON.stringify(base) }), {});
console.log(`${unbound.status === 503 ? 'ok  ' : 'FAIL'} without Analytics Engine: ${unbound.status}`);
ok &&= unbound.status === 503;
const speed = written.find((p) => p.blobs[5] === 'render');
console.log(`written: ${written.length} (want 7); a speed: ${JSON.stringify(speed)}`);
process.exit(ok && written.length === 7 && speed?.doubles[1] === 38 && g.status === 405 && other.status === 404 ? 0 : 1);
