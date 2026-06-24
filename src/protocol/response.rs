use bytes::{BufMut, Bytes, BytesMut};

/// Frame de response:
///   [1 byte status] [4 bytes payload_len] [payload_len bytes payload]
///
/// Status codes:
///   0x00 = OK      (com payload)
///   0x01 = NOT_FOUND
///   0x02 = ERROR   (payload = mensagem de erro UTF-8)
///   0x03 = PONG

pub const STATUS_OK: u8 = 0x00;
pub const STATUS_NOT_FOUND: u8 = 0x01;
pub const STATUS_ERROR: u8 = 0x02;
pub const STATUS_PONG: u8 = 0x03;

#[inline]
pub fn encode_pong() -> Bytes {
    let mut buf = BytesMut::with_capacity(5);
    buf.put_u8(STATUS_PONG);
    buf.put_u32(0);
    buf.freeze()
}

#[inline]
pub fn encode_ok(payload: &[u8]) -> Bytes {
    let mut buf = BytesMut::with_capacity(5 + payload.len());
    buf.put_u8(STATUS_OK);
    buf.put_u32(payload.len() as u32);
    buf.put_slice(payload);
    buf.freeze()
}

#[inline]
pub fn encode_not_found() -> Bytes {
    let mut buf = BytesMut::with_capacity(5);
    buf.put_u8(STATUS_NOT_FOUND);
    buf.put_u32(0);
    buf.freeze()
}

#[inline]
pub fn encode_error(msg: &str) -> Bytes {
    let b = msg.as_bytes();
    let mut buf = BytesMut::with_capacity(5 + b.len());
    buf.put_u8(STATUS_ERROR);
    buf.put_u32(b.len() as u32);
    buf.put_slice(b);
    buf.freeze()
}
