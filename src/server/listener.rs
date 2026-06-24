use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing::info;

use crate::cache::store::CacheStore;
use super::handler::handle_connection;

pub async fn run(addr: SocketAddr, store: CacheStore) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("WireCache escutando em {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;

        // TCP_NODELAY: elimina latência do algoritmo de Nagle para respostas pequenas
        let _ = stream.set_nodelay(true);

        let store = store.clone();
        tokio::spawn(async move {
            handle_connection(stream, store).await;
        });
    }
}
