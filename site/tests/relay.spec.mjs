// The relay between the studio demo and a phone (worker/room.js), run as Cloudflare runs it: a room passes a phone's
// questions to its host and the host's answers to that phone alone, tells each side who comes and goes, takes four
// phones, and only the site's own pages may join one.
import { test, expect } from "@playwright/test";
import worker from "../worker/index.js";
import { PHONES, Room } from "../worker/room.js";
import { Sockets } from "./rooms.mjs";

const ORIGIN = "https://claudius.devlato.com";
const NAME = "0123456789abcdef0123456789abcdef";

/** a WebSocket as a room has it: open, and what it was sent */
function socket() {
  return {
    readyState: 1,
    said: [],
    send(m) {
      this.said.push(m);
    },
    close() {
      this.readyState = 3;
    },
  };
}

function room() {
  const ctx = new Sockets();
  const r = new Room(ctx);
  r.phones = () => ctx.getWebSockets("phone").map((p) => ctx.getTags(p)[1]);
  return r;
}

test.describe("the relay", () => {
  test("passes a phone's questions to the host and the host's answers to that phone alone", () => {
    const r = room();
    const host = socket();
    const [a, b] = [socket(), socket()];
    r.join(host, "host");
    r.join(a, "phone");
    r.join(b, "phone");
    const [ia, ib] = r.phones();
    expect(host.said).toEqual([`${ia}\n{"event":"joined"}`, `${ib}\n{"event":"joined"}`]);
    r.webSocketMessage(a, '{"id":1,"method":"GET","path":"api/clock"}');
    expect(host.said.at(-1)).toBe(`${ia}\n{"id":1,"method":"GET","path":"api/clock"}`);
    r.webSocketMessage(host, `${ia}\n{"id":1,"status":200}`);
    expect([a.said, b.said]).toEqual([['{"id":1,"status":200}'], []]);
    // a sound: the phone's id, then what it is for and the sound itself
    const sound = new Uint8Array([...new TextEncoder().encode(ib), 7, 0, 0, 0, 82, 73, 70, 70]);
    r.webSocketMessage(host, sound.buffer);
    expect(new Uint8Array(b.said[0])).toEqual(new Uint8Array([7, 0, 0, 0, 82, 73, 70, 70]));
    // a phone only asks, in words
    r.webSocketMessage(a, new ArrayBuffer(40));
    expect(host.said).toHaveLength(3);
  });

  test("tells each side who comes and goes, and a tab opened again takes the room", () => {
    const r = room();
    const early = socket();
    r.join(early, "phone");
    expect(early.said, "no demo open yet").toEqual(['{"event":"gone"}']);
    const host = socket();
    r.join(host, "host");
    expect(host.said, "the phone that came first").toEqual([`${r.phones()[0]}\n{"event":"joined"}`]);
    const again = socket();
    r.join(again, "host");
    expect(host.readyState).toBe(3);
    expect(again.said).toHaveLength(1);
    r.webSocketClose(host);
    expect(early.said, "the room still has its host").toHaveLength(1);
    r.webSocketMessage(early, '{"id":2}');
    expect(again.said.at(-1)).toBe(`${r.phones()[0]}\n{"id":2}`);
    r.webSocketClose(early);
    expect(again.said.at(-1)).toBe(`${r.phones()[0]}\n{"event":"left"}`);
    const late = socket();
    r.join(late, "phone");
    again.readyState = 3;
    r.webSocketClose(again);
    expect(late.said).toEqual(['{"event":"gone"}']);
  });

  test("takes four phones, by WebSocket only", () => {
    const r = room();
    const ask = (as, upgrade = "websocket") =>
      r.refusal(new Request(`${ORIGIN}/relay/${NAME}?as=${as}`, { headers: { upgrade } }));
    expect(ask("phone")).toBeNull();
    for (let i = 0; i < PHONES; i++) r.join(socket(), "phone");
    expect(ask("phone").status).toBe(429);
    expect(ask("host")).toBeNull();
    expect(ask("anyone").status).toBe(400);
    expect(ask("host", "h2c").status).toBe(426);
  });

  test("lets only the site's own pages into a room", async () => {
    const joined = [];
    const env = {
      ROOMS: {
        idFromName: (n) => `id of ${n}`,
        get: (id) => ({ fetch: async () => (joined.push(id), new Response("joined")) }),
      },
    };
    const get = (path, origin) =>
      worker.fetch(new Request(ORIGIN + path, { headers: origin ? { origin } : {} }), env, { waitUntil() {} });
    expect((await get(`/relay/${NAME}?as=phone`, ORIGIN)).status).toBe(200);
    expect(joined).toEqual([`id of ${NAME}`]);
    expect((await get(`/relay/${NAME}?as=phone`, "https://elsewhere.example")).status).toBe(403);
    expect((await get(`/relay/${NAME}?as=host`)).status).toBe(403);
    expect((await get("/relay/0123?as=phone", ORIGIN)).status).toBe(404);
    expect((await get(`/relay/${NAME.toUpperCase()}?as=phone`, ORIGIN)).status).toBe(404);
    expect(joined).toHaveLength(1);
  });
});
