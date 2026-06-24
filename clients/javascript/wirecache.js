/**
 * WireCache JavaScript/Node.js Client
 * Protocolo binário TCP — sem dependências externas.
 *
 * Frame request:  [1B op][4B key_len][4B val_len][4B ttl][key][value]
 * Frame response: [1B status][4B payload_len][payload]
 */

const net = require("net");

const OP = { PING: 0x01, SET: 0x02, GET: 0x03, DEL: 0x04, FLUSH: 0x05, STATS: 0x06 };
const STATUS = { OK: 0x00, NOT_FOUND: 0x01, ERROR: 0x02, PONG: 0x03 };

class WireCacheClient {
  constructor(host = "127.0.0.1", port = 6380) {
    this._host = host;
    this._port = port;
    this._socket = null;
    this._buf = Buffer.alloc(0);
    this._pending = [];
  }

  connect() {
    return new Promise((resolve, reject) => {
      this._socket = net.createConnection({ host: this._host, port: this._port }, () => {
        this._socket.setNoDelay(true);
        resolve();
      });
      this._socket.on("data", (chunk) => this._onData(chunk));
      this._socket.on("error", reject);
      this._socket.on("close", () => {
        for (const { reject } of this._pending) reject(new Error("Conexão fechada"));
        this._pending = [];
      });
    });
  }

  disconnect() {
    this._socket?.destroy();
  }

  ping() {
    return this._cmd(this._frame(OP.PING, "", "", 0)).then(
      ({ status }) => status === STATUS.PONG
    );
  }

  set(key, value, ttl = 0) {
    return this._cmd(this._frame(OP.SET, key, value, ttl)).then(({ status, payload }) => {
      if (status === STATUS.ERROR) throw new Error(payload.toString());
      return status === STATUS.OK;
    });
  }

  get(key) {
    return this._cmd(this._frame(OP.GET, key, "", 0)).then(({ status, payload }) => {
      if (status === STATUS.NOT_FOUND) return null;
      if (status === STATUS.ERROR) throw new Error(payload.toString());
      return payload;
    });
  }

  delete(key) {
    return this._cmd(this._frame(OP.DEL, key, "", 0)).then(
      ({ status, payload }) => status === STATUS.OK && payload.toString() === "1"
    );
  }

  flush() {
    return this._cmd(this._frame(OP.FLUSH, "", "", 0)).then(
      ({ status }) => status === STATUS.OK
    );
  }

  stats() {
    return this._cmd(this._frame(OP.STATS, "", "", 0)).then(({ status, payload }) => {
      if (status !== STATUS.OK) throw new Error("Falha ao obter stats");
      return JSON.parse(payload.toString());
    });
  }

  // ------------------------------------------------------------------ //
  // Internos
  // ------------------------------------------------------------------ //

  _frame(op, key, value, ttl) {
    const k = Buffer.isBuffer(key) ? key : Buffer.from(key);
    const v = Buffer.isBuffer(value) ? value : Buffer.from(value);
    const header = Buffer.alloc(13);
    header.writeUInt8(op, 0);
    header.writeUInt32BE(k.length, 1);
    header.writeUInt32BE(v.length, 5);
    header.writeUInt32BE(ttl, 9);
    return Buffer.concat([header, k, v]);
  }

  _cmd(frame) {
    return new Promise((resolve, reject) => {
      this._pending.push({ resolve, reject });
      this._socket.write(frame);
    });
  }

  _onData(chunk) {
    this._buf = Buffer.concat([this._buf, chunk]);
    while (this._buf.length >= 5) {
      const payloadLen = this._buf.readUInt32BE(1);
      const total = 5 + payloadLen;
      if (this._buf.length < total) break;
      const status = this._buf.readUInt8(0);
      const payload = this._buf.slice(5, total);
      this._buf = this._buf.slice(total);
      const entry = this._pending.shift();
      if (entry) entry.resolve({ status, payload });
    }
  }
}

module.exports = { WireCacheClient };

// Exemplo de uso rápido (node wirecache.js)
if (require.main === module) {
  (async () => {
    const client = new WireCacheClient();
    await client.connect();
    console.log("PING:", await client.ping());
    await client.set("chave", "valor", 60);
    console.log("GET:", (await client.get("chave"))?.toString());
    console.log("STATS:", await client.stats());
    client.disconnect();
  })();
}
