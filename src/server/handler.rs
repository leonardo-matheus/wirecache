use std::time::Instant;

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
    let peer_str = peer
        .map(|a| a.to_string())
        .unwrap_or_else(|| "?".to_string());

    println!("[CONN ] + {}", peer_str);

    let mut buf = Vec::with_capacity(READ_BUF_SIZE);
    let mut tmp = [0u8; READ_BUF_SIZE];

    loop {
        match stream.read(&mut tmp).await {
            Ok(0) => {
                println!("[CONN ] - {}", peer_str);
                break;
            }
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(e) => {
                warn!("erro de leitura de {}: {}", peer_str, e);
                break;
            }
        }

        let mut consumed = 0;
        loop {
            match parse_frame(&buf[consumed..]) {
                Ok((cmd, size)) => {
                    consumed += size;
                    let t0 = Instant::now();
                    let response = dispatch(&cmd, &store).await;
                    let us = t0.elapsed().as_micros();
                    log_request(&peer_str, &cmd, us);
                    if let Err(e) = stream.write_all(&response).await {
                        warn!("erro de escrita para {}: {}", peer_str, e);
                        return;
                    }
                }
                Err(ParseError::Incomplete) => break,
                Err(ParseError::InvalidOpcode(op)) => {
                    println!("[ERROR] {} opcode inválido: 0x{:02X}", peer_str, op);
                    let _ = stream
                        .write_all(&encode_error(&format!("opcode inválido: 0x{:02X}", op)))
                        .await;
                    consumed += 1;
                }
                Err(ParseError::Malformed) => {
                    println!("[ERROR] {} frame mal-formado", peer_str);
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

fn log_request(peer: &str, cmd: &Command, us: u128) {
    match cmd {
        Command::Ping =>
            println!("[PING ] {} | {:.3} ms", peer, us as f64 / 1000.0),
        Command::Set { key, value, ttl_secs } => {
            let ttl_info = if *ttl_secs > 0 { format!(" ttl={}s", ttl_secs) } else { String::new() };
            println!(
                "[SET  ] {} | key={:?} size={}B{} | {:.3} ms",
                peer,
                key_str(key),
                value.len(),
                ttl_info,
                us as f64 / 1000.0,
            );
        }
        Command::Get { key } =>
            println!("[GET  ] {} | key={:?} | {:.3} ms", peer, key_str(key), us as f64 / 1000.0),
        Command::Del { key } =>
            println!("[DEL  ] {} | key={:?} | {:.3} ms", peer, key_str(key), us as f64 / 1000.0),
        Command::Flush =>
            println!("[FLUSH] {} | {:.3} ms", peer, us as f64 / 1000.0),
        Command::Stats =>
            println!("[STATS] {} | {:.3} ms", peer, us as f64 / 1000.0),
    }
}

/// Converte bytes da chave para string legível (trunca em 40 chars).
fn key_str(key: &Bytes) -> String {
    let s = String::from_utf8_lossy(key);
    if s.len() > 40 {
        format!("{}…", &s[..40])
    } else {
        s.into_owned()
    }
}

async fn dispatch(cmd: &Command, store: &CacheStore) -> Bytes {
    match cmd {
        Command::Ping => encode_pong(),

        Command::Set { key, value, ttl_secs } => {
            store.set(key.clone(), value.clone(), *ttl_secs).await;
            encode_ok(b"OK")
        }

        Command::Get { key } => match store.get(key).await {
            Some(val) => encode_ok(&val),
            None => encode_not_found(),
        },

        Command::Del { key } => {
            let existed = store.del(key).await;
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
