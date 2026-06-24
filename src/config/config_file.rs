use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

const DEFAULT_ADDR: &str = "0.0.0.0:6380";
const DEFAULT_MAX_CAPACITY: u64 = 1_000_000;

#[derive(Debug, Deserialize, PartialEq)]
pub struct AppConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub max_capacity: Option<u64>,
    pub metrics_interval_secs: Option<u64>,
    pub single_thread_mode: Option<bool>,
    pub debug: Option<bool>,
}

impl AppConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        let host = self.host.as_deref().unwrap_or("0.0.0.0");
        let port = self.port.unwrap_or(6380);
        format!("{}:{}", host, port).parse().unwrap_or_else(|_| {
            DEFAULT_ADDR.parse().unwrap()
        })
    }

    pub fn max_capacity(&self) -> u64 {
        self.max_capacity.unwrap_or(DEFAULT_MAX_CAPACITY)
    }

    pub fn metrics_interval(&self) -> u64 {
        self.metrics_interval_secs.unwrap_or(5)
    }
}

pub fn verify_config_file_exists<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).is_ok()
}

pub fn load_config(path: &str) -> Result<AppConfig, String> {
    if !verify_config_file_exists(path) {
        return Err(format!("Arquivo de configuração '{}' não encontrado.", path));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler o arquivo: {}", e))?;

    let config: AppConfig = toml::from_str(&content)
        .map_err(|e| format!("Falha ao parsear TOML: {}", e))?;

    Ok(config)
}

pub fn load_config_or_default(path: &str) -> AppConfig {
    match load_config(path) {
        Ok(cfg) => cfg,
        Err(_) => AppConfig {
            host: None,
            port: None,
            max_capacity: None,
            metrics_interval_secs: None,
            single_thread_mode: None,
            debug: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_verify_config_file_exists_success() {
        let path = "test_config_existing.toml";
        File::create(path).unwrap();
        assert!(verify_config_file_exists(path));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_verify_config_file_exists_fail() {
        assert!(!verify_config_file_exists("non_existent_file_12345.toml"));
    }

    #[test]
    fn test_load_config_success() {
        let path = "wirecache_test_success.toml";
        let mut f = File::create(path).unwrap();
        f.write_all(b"single_thread_mode = true\ndebug = false\nport = 6380\n").unwrap();

        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.single_thread_mode, Some(true));
        assert_eq!(cfg.debug, Some(false));
        assert_eq!(cfg.port, Some(6380));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_load_config_optional_fields_missing() {
        let path = "wirecache_test_missing_fields.toml";
        File::create(path).unwrap();

        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.single_thread_mode, None);
        assert_eq!(cfg.debug, None);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_load_config_file_not_found() {
        let err = load_config("wirecache_missing.toml").unwrap_err();
        assert!(err.contains("não encontrado"));
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let path = "wirecache_test_invalid.toml";
        let mut f = File::create(path).unwrap();
        f.write_all(b"single_thread_mode = \"not_a_bool\"\n").unwrap();

        let err = load_config(path).unwrap_err();
        assert!(err.contains("Falha ao parsear"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_bind_addr_defaults() {
        let cfg = load_config_or_default("nonexistent.toml");
        assert_eq!(cfg.bind_addr().port(), 6380);
    }

    #[test]
    fn test_max_capacity_default() {
        let cfg = load_config_or_default("nonexistent.toml");
        assert_eq!(cfg.max_capacity(), 1_000_000);
    }
}
