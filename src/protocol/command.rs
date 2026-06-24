use bytes::Bytes;

/// Protocolo binário do SWCache (TCP)
///
/// Frame de request:
///   [1 byte opcode] [4 bytes key_len] [4 bytes val_len] [4 bytes ttl_secs] [key_len bytes key] [val_len bytes value]
///
/// ttl_secs = 0 significa sem expiração.
/// val_len  = 0 em comandos sem valor (GET, DEL, PING, FLUSH, STATS).
///
/// Opcodes:
///   0x01 = PING
///   0x02 = SET
///   0x03 = GET
///   0x04 = DEL
///   0x05 = FLUSH
///   0x06 = STATS

pub const OP_PING: u8 = 0x01;
pub const OP_SET: u8 = 0x02;
pub const OP_GET: u8 = 0x03;
pub const OP_DEL: u8 = 0x04;
pub const OP_FLUSH: u8 = 0x05;
pub const OP_STATS: u8 = 0x06;

/// Tamanho mínimo de um frame: 1 (op) + 4 (key_len) + 4 (val_len) + 4 (ttl) = 13 bytes
pub const FRAME_HEADER_SIZE: usize = 13;

#[derive(Debug, Clone)]
pub enum Command {
    Ping,
    Set { key: Bytes, value: Bytes, ttl_secs: u32 },
    Get { key: Bytes },
    Del { key: Bytes },
    Flush,
    Stats,
}

#[derive(Debug)]
pub enum ParseError {
    Incomplete,
    InvalidOpcode(u8),
    Malformed,
}

/// Tenta parsear um frame do buffer. Retorna `(Command, bytes_consumed)` ou erro.
pub fn parse_frame(buf: &[u8]) -> Result<(Command, usize), ParseError> {
    if buf.len() < FRAME_HEADER_SIZE {
        return Err(ParseError::Incomplete);
    }

    let opcode = buf[0];
    let key_len = u32::from_be_bytes(buf[1..5].try_into().unwrap()) as usize;
    let val_len = u32::from_be_bytes(buf[5..9].try_into().unwrap()) as usize;
    let ttl_secs = u32::from_be_bytes(buf[9..13].try_into().unwrap());

    let total = FRAME_HEADER_SIZE + key_len + val_len;
    if buf.len() < total {
        return Err(ParseError::Incomplete);
    }

    let key_start = FRAME_HEADER_SIZE;
    let val_start = key_start + key_len;

    let key = Bytes::copy_from_slice(&buf[key_start..val_start]);
    let value = Bytes::copy_from_slice(&buf[val_start..val_start + val_len]);

    let cmd = match opcode {
        OP_PING => Command::Ping,
        OP_SET => {
            if key.is_empty() {
                return Err(ParseError::Malformed);
            }
            Command::Set { key, value, ttl_secs }
        }
        OP_GET => {
            if key.is_empty() {
                return Err(ParseError::Malformed);
            }
            Command::Get { key }
        }
        OP_DEL => {
            if key.is_empty() {
                return Err(ParseError::Malformed);
            }
            Command::Del { key }
        }
        OP_FLUSH => Command::Flush,
        OP_STATS => Command::Stats,
        other => return Err(ParseError::InvalidOpcode(other)),
    };

    Ok((cmd, total))
}
