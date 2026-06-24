# WireCache

High-performance in-memory cache server written in Rust, designed to compete with Redis. Uses a lightweight binary TCP protocol compatible with any language — no Redis client dependency required.

---

## Features

- **Binary TCP protocol** — compact, low-overhead frames over raw TCP
- **Async I/O** via Tokio — one lightweight task per connection
- **LRU eviction** via Moka (similar to Java's Caffeine) — concurrent, lock-free
- **Per-entry TTL** — zero-expiry means no expiration
- **Atomic statistics** — hits, misses, sets, deletes, flushes, hit rate
- **TOML configuration** — optional file with sane defaults
- **Client libraries** included for Rust, Python, JavaScript and Java

---

## Quickstart

```bash
cargo install wirecache
wirecache
```

By default the server listens on `0.0.0.0:6380`.

### Configuration (`swconfig.toml`)

```toml
host         = "0.0.0.0"
port         = 6380
max_capacity = 1_000_000
debug        = false
```

---

## Protocol

WireCache uses a custom binary TCP protocol. Each request and response is a self-contained frame — no newlines, no string parsing.

### Request frame

```
[1 byte  opcode  ]
[4 bytes key_len ] big-endian u32
[4 bytes val_len ] big-endian u32
[4 bytes ttl_secs] big-endian u32  (0 = no expiry)
[key_len bytes   ] raw key bytes
[val_len bytes   ] raw value bytes
```

### Response frame

```
[1 byte  status     ]
[4 bytes payload_len] big-endian u32
[payload_len bytes  ] raw payload
```

### Opcodes

| Hex    | Command | Description                          |
|--------|---------|--------------------------------------|
| `0x01` | PING    | Health check                         |
| `0x02` | SET     | Store key/value with optional TTL    |
| `0x03` | GET     | Retrieve value by key                |
| `0x04` | DEL     | Delete key                           |
| `0x05` | FLUSH   | Invalidate all entries               |
| `0x06` | STATS   | JSON snapshot of server statistics   |

### Status codes

| Hex    | Meaning   | Payload            |
|--------|-----------|--------------------|
| `0x00` | OK        | response value     |
| `0x01` | NOT_FOUND | empty              |
| `0x02` | ERROR     | UTF-8 error string |
| `0x03` | PONG      | empty              |

---

## Client Libraries

Minimal implementations included — no external dependencies in any of them.

### Python

```python
from clients.python.wirecache import WireCacheClient

with WireCacheClient("127.0.0.1", 6380) as c:
    c.ping()                      # True
    c.set("key", "value", ttl=60)
    c.get("key")                  # b"value"
    c.delete("key")               # True
    c.stats()                     # dict with hits, misses, hit_rate_pct, ...
```

### JavaScript (Node.js)

```js
const { WireCacheClient } = require("./clients/javascript/wirecache");

const c = new WireCacheClient();
await c.connect();
await c.set("key", "value", 60);
const val = await c.get("key");   // Buffer
await c.disconnect();
```

### Java

```java
try (WireCacheClient c = new WireCacheClient("127.0.0.1", 6380)) {
    c.ping();
    c.set("key", "value", 60);
    String val = c.getString("key");
    System.out.println(c.stats());
}
```

### Rust

```rust
use wirecache::protocol::codec::{encode_set, encode_get};
// connect via tokio TcpStream and send/recv frames directly
```

---

## Benchmarks

Measured locally with [Criterion](https://github.com/bheisler/criterion.rs) — 100 samples, 3 s warm-up, `--release` build with LTO.

**Environment:** Windows 11, Tokio multi-thread runtime, loopback TCP (`127.0.0.1:6380`), `TCP_NODELAY` enabled.

> Latency is dominated by loopback TCP round-trip (~45–55 µs). On Linux with Unix sockets or io_uring, expect sub-10 µs.

### Latency (p50 / median)

| Operation     | Value size | Median latency | Ops/sec   |
|---------------|-----------|----------------|-----------|
| PING          | —         | 53.5 µs        | ~18,700   |
| SET           | 16 B      | 50.8 µs        | ~19,700   |
| GET (hit)     | 16 B      | 49.8 µs        | ~20,100   |
| SET           | 128 B     | 51.7 µs        | ~19,300   |
| GET (hit)     | 128 B     | 45.5 µs        | ~22,000   |
| SET           | 1 KB      | 46.7 µs        | ~21,400   |

### Throughput (saturated, single connection)

| Operation     | Value size | Throughput  |
|---------------|-----------|-------------|
| SET           | 16 B      | 307 KiB/s   |
| GET (hit)     | 16 B      | 314 KiB/s   |
| SET           | 128 B     | 2.36 MiB/s  |
| GET (hit)     | 128 B     | 2.68 MiB/s  |
| SET           | 1 KB      | 20.9 MiB/s  |

> Throughput scales linearly with value size since latency is flat (~47 µs) across all sizes — confirming the bottleneck is TCP round-trip, not cache overhead.

### Run benchmarks yourself

```bash
cargo bench
# HTML reports at: target/criterion/report/index.html
```

---

## Architecture

```
src/
├── main.rs              # Entry point — loads config, starts server
├── lib.rs               # Public API for benchmarks and clients
├── cache/
│   ├── store.rs         # CacheStore (Moka LRU + per-entry TTL)
│   └── stats.rs         # Atomic counters (hits, misses, sets, ...)
├── protocol/
│   ├── command.rs       # Frame parser — parse_frame() -> Command
│   ├── response.rs      # Response encoders
│   └── codec.rs         # Client-side frame builders
├── server/
│   ├── listener.rs      # TcpListener loop — spawns one task per connection
│   └── handler.rs       # Per-connection state machine + dispatch
├── config/
│   └── config_file.rs   # TOML config loading
└── disclaimer/
    ├── logo.rs           # ASCII art
    ├── beta.rs           # Startup banner + system info
    └── system_info.rs    # CPU / RAM via sysinfo
```

### Request lifecycle

```
Client ──TCP──► listener::run()
                  └─ tokio::spawn(handler::handle_connection)
                       ├─ read bytes into Vec<u8>
                       ├─ parse_frame() → Command (zero-copy Bytes)
                       ├─ dispatch() → CacheStore operation
                       └─ write response frame
```

---

## License

MIT
