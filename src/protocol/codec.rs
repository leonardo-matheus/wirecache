/// Helpers para serializar comandos no lado cliente (Rust client).
use bytes::{BufMut, Bytes, BytesMut};
use super::command::{OP_DEL, OP_FLUSH, OP_GET, OP_PING, OP_SET, OP_STATS};

#[inline]
fn frame(op: u8, key: &[u8], value: &[u8], ttl: u32) -> Bytes {
    let total = 13 + key.len() + value.len();
    let mut buf = BytesMut::with_capacity(total);
    buf.put_u8(op);
    buf.put_u32(key.len() as u32);
    buf.put_u32(value.len() as u32);
    buf.put_u32(ttl);
    buf.put_slice(key);
    buf.put_slice(value);
    buf.freeze()
}

pub fn encode_ping() -> Bytes {
    frame(OP_PING, b"", b"", 0)
}

pub fn encode_set(key: &[u8], value: &[u8], ttl_secs: u32) -> Bytes {
    frame(OP_SET, key, value, ttl_secs)
}

pub fn encode_get(key: &[u8]) -> Bytes {
    frame(OP_GET, key, b"", 0)
}

pub fn encode_del(key: &[u8]) -> Bytes {
    frame(OP_DEL, key, b"", 0)
}

pub fn encode_flush() -> Bytes {
    frame(OP_FLUSH, b"", b"", 0)
}

pub fn encode_stats() -> Bytes {
    frame(OP_STATS, b"", b"", 0)
}
