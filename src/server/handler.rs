use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, warn};

use crate::cache::store::CacheStore;
use crate::protocol::command::{parse_frame, Command, ParseError};
use crate::protocol::response::{encode_error, encode_not_found, encode_ok, encode_pong};

const READ_BUF_SIZE: usize = 64 * 1024;

pub async fn handle_connection(mut stream: TcpStream, store: CacheStore) {
    let peer = stream.peer_addr().ok();
    debug!(?peer, "conexão aceita");

    let mut buf = Vec::with_capacity(READ_BUF_SIZE);
    let mut tmp = [0u8; READ_BUF_SIZE];

    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => {
                debug!(?peer, "cliente desconectou");
                break;
            }
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(e) => {
                warn!(?peer, "erro de leitura: {}", e);
                break;
            }
        }

        let mut consumed = 0;
        loop {
            match parse_frame(&buf[consumed..]) {
                Ok((cmd, size)) => {
                    consumed += size;
                    let response = dispatch(cmd, &store).await;
                    if let Err(e) = stream.write_all(&response).await {
                        warn!(?peer, "erro de escrita: {}", e);
                        return;
                    }
                }
                Err(ParseError::Incomplete) => break,
                Err(ParseError::InvalidOpcode(op)) => {
                    let _ = stream
                        .write_all(&encode_error(&format!("opcode inválido: 0x{:02X}", op)))
                        .await;
                    consumed += 1;
                }
                Err(ParseError::Malformed) => {
                    let _ = stream.write_all(&encode_error("frame mal-formado")).await;
                    break;
                }
            }
        }

        if consumed > 0 {
            buf.drain(..consumed);
        }
    }
}

async fn dispatch(cmd: Command, store: &CacheStore) -> Bytes {
    match cmd {
        Command::Ping => encode_pong(),

        Command::Set { key, value, ttl_secs } => {
            store.set(key, value, ttl_secs).await;
            encode_ok(b"OK")
        }

        Command::Get { key } => match store.get(&key).await {
            Some(val) => encode_ok(&val),
            None => encode_not_found(),
        },

        Command::Del { key } => {
            let existed = store.del(&key).await;
            encode_ok(if existed { b"1" } else { b"0" })
        }

        Command::Flush => {
            store.flush().await;
            encode_ok(b"OK")
        }

        Command::Stats => {
            let snap = store.stats.snapshot();
            let entries = store.entry_count();
            let json = format!(
                r#"{{"entries":{},"hits":{},"misses":{},"sets":{},"deletes":{},"flushes":{},"hit_rate_pct":{:.2}}}"#,
                entries,
                snap.hits,
                snap.misses,
                snap.sets,
                snap.deletes,
                snap.flushes,
                snap.hit_rate
            );
            encode_ok(json.as_bytes())
        }
    }
}
