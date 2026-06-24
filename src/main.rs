use config::config_file::load_config_or_default;
use disclaimer::{beta, logo};
use utils::clean_terminal;

mod cache;
mod config;
mod disclaimer;
mod protocol;
mod runtime;
mod server;
mod utils;

#[tokio::main]
async fn main() {
    startup();

    let cfg = load_config_or_default("swconfig.toml");

    let level = if cfg.debug.unwrap_or(false) { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .init();

    let addr = cfg.bind_addr();
    let capacity = cfg.max_capacity();

    let store = cache::store::CacheStore::new(capacity);

    server::listener::run(addr, store)
        .await
        .expect("Falha ao iniciar o servidor");
}

fn startup() {
    clean_terminal::clean();
    logo::show_logo();
    beta::beta_warning();
}
