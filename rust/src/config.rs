use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub postgresql: PostgresConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgresConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_database")]
    pub database: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_pool_size")]
    pub max_connections: u32,
}

fn default_host() -> String { "localhost".to_string() }
fn default_port() -> u16 { 5432 }
fn default_database() -> String { "bountycatch".to_string() }
fn default_user() -> String { "postgres".to_string() }
fn default_pool_size() -> u32 { 10 }

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            database: default_database(),
            user: default_user(),
            password: String::new(),
            max_connections: default_pool_size(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            postgresql: PostgresConfig::default(),
        }
    }
}

impl Config {
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let config_file = if let Some(path) = config_path {
            Some(path.to_path_buf())
        } else {
            Self::find_config_file()
        };

        let mut config = if let Some(path) = config_file {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file: {:?}", path))?;
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {:?}", path))?
        } else {
            Config::default()
        };

        // Environment variable overrides
        if let Ok(host) = std::env::var("PGHOST") {
            config.postgresql.host = host;
        }
        if let Ok(port) = std::env::var("PGPORT") {
            if let Ok(p) = port.parse() {
                config.postgresql.port = p;
            }
        }
        if let Ok(db) = std::env::var("PGDATABASE") {
            config.postgresql.database = db;
        }
        if let Ok(user) = std::env::var("PGUSER") {
            config.postgresql.user = user;
        }
        if let Ok(pass) = std::env::var("PGPASSWORD") {
            config.postgresql.password = pass;
        }

        Ok(config)
    }

    fn find_config_file() -> Option<PathBuf> {
        let search_paths: Vec<PathBuf> = vec![
            dirs::config_dir().map(|p| p.join("bountycatch/config.json")),
            dirs::home_dir().map(|p| p.join(".bountycatch/config.json")),
            Some(PathBuf::from("/etc/bountycatch/config.json")),
            std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("config.json"))),
            Some(PathBuf::from("config.json")),
        ]
        .into_iter()
        .flatten()
        .collect();

        for path in search_paths {
            if path.exists() {
                return Some(path);
            }
        }
        None
    }
}
