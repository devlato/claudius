// The relay's rooms as Cloudflare runs them, for the test server (serve.mjs): the Worker's own `Room` (worker/room.js),
// given a Durable Object's WebSockets (accept, tags, lookup) and real WebSockets, which Node does not serve, so this
// speaks just enough of RFC 6455 for a browser: the handshake, text and binary messages (in fragments too), ping and
// close. A room lasts while the server runs.

import { createHash } from "node:crypto";
import { Room } from "../worker/room.js";
import { room as roomOf } from "../worker/index.js";

/** a Durable Object's state, as a room uses it: the WebSockets it accepted and their tags */
export class Sockets {
  constructor() {
    this.all = [];
  }
  acceptWebSocket(ws, tags) {
    this.all.push({ ws, tags });
  }
  getWebSockets(tag) {
    return this.all.filter((s) => tag === undefined || s.tags.includes(tag)).map((s) => s.ws);
  }
  getTags(ws) {
    return this.all.find((s) => s.ws === ws)?.tags ?? [];
  }
  drop(ws) {
    this.all = this.all.filter((s) => s.ws !== ws);
  }
}

const rooms = new Map();

function frame(op, payload) {
  const n = payload.length;
  const head = n < 126 ? Buffer.alloc(2) : n < 65536 ? Buffer.alloc(4) : Buffer.alloc(10);
  head[0] = 0x80 | op;
  if (n < 126) head[1] = n;
  else if (n < 65536) {
    head[1] = 126;
    head.writeUInt16BE(n, 2);
  } else {
    head[1] = 127;
    head.writeBigUInt64BE(BigInt(n), 2);
  }
  return Buffer.concat([head, payload]);
}

/** an upgrade request for `/relay/<room>`, answered as the Worker and its room answer it */
export async function upgrade(req, socket, origin) {
  const headers = { origin: req.headers.origin ?? "", upgrade: req.headers.upgrade ?? "" };
  const request = new Request(new URL(req.url, origin), { headers });
  const name = roomOf(request);
  const as = new URL(request.url).searchParams.get("as");
  const refuse = (status, text) => {
    socket.end(`HTTP/1.1 ${status} No\r\ncontent-type: text/plain\r\ncontent-length: ${Buffer.byteLength(text)}\r\n\r\n${text}`);
  };
  if (typeof name !== "string") return refuse(name.status, await name.text());
  if (!rooms.has(name)) {
    const ctx = new Sockets();
    rooms.set(name, { ctx, room: new Room(ctx) });
  }
  const { ctx, room } = rooms.get(name);
  const no = room.refusal(request);
  if (no) return refuse(no.status, await no.text());
  const key = req.headers["sec-websocket-key"];
  const accept = createHash("sha1").update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest("base64");
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  const ws = {
    readyState: 1,
    send(data) {
      if (this.readyState !== 1) throw new Error("closed");
      const bytes = typeof data === "string" ? Buffer.from(data) : Buffer.from(data instanceof ArrayBuffer ? data : data.buffer);
      socket.write(frame(typeof data === "string" ? 1 : 2, bytes));
    },
    close(code = 1000, reason = "") {
      if (this.readyState !== 1) return;
      this.readyState = 2;
      const body = Buffer.alloc(2 + Buffer.byteLength(reason));
      body.writeUInt16BE(code, 0);
      body.write(reason, 2);
      socket.end(frame(8, body));
    },
  };
  let closed = false;
  const gone = () => {
    if (closed) return;
    closed = true;
    ws.readyState = 3;
    room.webSocketClose(ws);
    ctx.drop(ws);
  };
  room.join(ws, as);
  let buf = Buffer.alloc(0);
  let parts = [];
  let kind = 0;
  socket.on("data", (d) => {
    buf = Buffer.concat([buf, d]);
    for (;;) {
      if (buf.length < 2) return;
      const fin = buf[0] & 0x80;
      const op = buf[0] & 0x0f;
      let len = buf[1] & 0x7f;
      let at = 2;
      if (len === 126) {
        if (buf.length < 4) return;
        len = buf.readUInt16BE(2);
        at = 4;
      } else if (len === 127) {
        if (buf.length < 10) return;
        len = Number(buf.readBigUInt64BE(2));
        at = 10;
      }
      const masked = buf[1] & 0x80;
      const mask = masked ? buf.subarray(at, at + 4) : null;
      if (masked) at += 4;
      if (buf.length < at + len) return;
      const payload = Buffer.from(buf.subarray(at, at + len));
      if (mask) for (let i = 0; i < len; i++) payload[i] ^= mask[i & 3];
      buf = buf.subarray(at + len);
      if (op === 8) {
        ws.close();
        gone();
        return;
      }
      if (op === 9) socket.write(frame(10, payload));
      if (op > 2) continue;
      if (op !== 0) kind = op;
      parts.push(payload);
      if (!fin) continue;
      const all = Buffer.concat(parts);
      parts = [];
      room.webSocketMessage(ws, kind === 1 ? all.toString("utf8") : all.buffer.slice(all.byteOffset, all.byteOffset + all.length));
    }
  });
  socket.on("close", gone);
  socket.on("error", gone);
}
