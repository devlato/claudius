// The relay between the studio demo and a phone (`/relay/<room>`). The demo runs claudius in a browser tab, which a
// phone cannot reach, so both open a WebSocket to the same room here, and this Durable Object passes their messages
// on. A room is one demo tab (the host) and the phones that scanned its code; its name is 128 random bits the tab
// makes, which only its QR code carries. The room keeps nothing: what passes through it goes straight on, and while
// nobody speaks it sleeps (the WebSocket Hibernation API), its sockets' tags saying who is who.
//
// A phone's messages go to the host as `<phone>\n<message>`. The host's text answers come as `<phone>\n<message>` and
// go to that phone as `<message>`; its binary answers (sounds) carry the phone's 36-character id first. The host hears
// `{"event":"joined"}` and `{"event":"left"}` from each phone (every phone there again when it comes back); a phone
// hears `{"event":"gone"}` when the room has no host, and `{"event":"back"}` when one joins, and what it asks between
// the two goes nowhere.

/** the most phones a room takes */
export const PHONES = 4;

const GONE = '{"event":"gone"}';
const BACK = '{"event":"back"}';

/** what `ws` is told, unless it has closed meanwhile */
function send(ws, message) {
  try {
    ws?.send(message);
  } catch {
    // it went as the message was on its way: its close says so to the others
  }
}

const plain = (body, status) =>
  new Response(body, { status, headers: { "content-type": "text/plain; charset=utf-8", "x-content-type-options": "nosniff" } });

export class Room {
  constructor(ctx) {
    this.ctx = ctx;
  }

  async fetch(request) {
    const no = this.refusal(request);
    if (no) return no;
    const [client, server] = Object.values(new WebSocketPair());
    this.join(server, new URL(request.url).searchParams.get("as"));
    return new Response(null, { status: 101, webSocket: client });
  }

  /** why the room turns `request` away, if it does */
  refusal(request) {
    if (request.headers.get("upgrade")?.toLowerCase() !== "websocket") return plain("a room is joined with a WebSocket", 426);
    const as = new URL(request.url).searchParams.get("as");
    if (as !== "host" && as !== "phone") return plain("a room is joined as=host or as=phone", 400);
    if (as === "phone" && this.ctx.getWebSockets("phone").length >= PHONES) return plain(`a room takes ${PHONES} phones`, 429);
    return null;
  }

  /** `ws` in the room as its host (in place of one there) or as a phone */
  join(ws, as) {
    if (as === "host") {
      for (const old of this.ctx.getWebSockets("host")) old.close(4000, "the demo was opened again in another tab");
      this.ctx.acceptWebSocket(ws, ["host"]);
      for (const p of this.ctx.getWebSockets("phone")) {
        send(ws, `${this.ctx.getTags(p)[1]}\n{"event":"joined"}`);
        send(p, BACK);
      }
      return;
    }
    const id = crypto.randomUUID();
    this.ctx.acceptWebSocket(ws, ["phone", id]);
    const host = this.host();
    send(host ?? ws, host ? `${id}\n{"event":"joined"}` : GONE);
  }

  /** the room's host, if it has one: the one tab of it still open */
  host(but) {
    return this.ctx.getWebSockets("host").find((h) => h !== but && h.readyState === 1);
  }

  webSocketMessage(ws, message) {
    const [role, id] = this.ctx.getTags(ws);
    if (role === "phone") {
      // a phone only asks; with no host there, its question goes unanswered and it says the demo is not open
      if (typeof message === "string") send(this.host(), `${id}\n${message}`);
      return;
    }
    if (typeof message === "string") {
      const cut = message.indexOf("\n");
      if (cut > 0) send(this.ctx.getWebSockets(message.slice(0, cut))[0], message.slice(cut + 1));
      return;
    }
    if (message.byteLength < 36) return;
    const to = new TextDecoder().decode(new Uint8Array(message, 0, 36));
    send(this.ctx.getWebSockets(to)[0], message.slice(36));
  }

  webSocketClose(ws) {
    const [role, id] = this.ctx.getTags(ws);
    if (role === "host") {
      if (!this.host(ws)) for (const p of this.ctx.getWebSockets("phone")) send(p, GONE);
    } else {
      send(this.host(), `${id}\n{"event":"left"}`);
    }
  }

  webSocketError(ws) {
    this.webSocketClose(ws);
  }
}
