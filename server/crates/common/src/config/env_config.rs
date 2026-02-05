use neocrates::{logger::LogConfig, serde::Deserialize};

#[derive(Debug, Deserialize, Clone)]
pub struct EnvConfig {
    #[serde(flatten)]
    pub log: LogConfig,

    pub server: ServerConfig,

    #[serde(rename = "pg-database")]
    pub pg_database: DatabaseConfig,

    pub redis: RedisConfig,

    pub apalis: ApalisConfig,

    #[serde(rename = "ignore-urls")]
    pub ignore_urls: Vec<String>,

    // pms-ignore-urls
    #[serde(rename = "pms-ignore-urls")]
    pub pms_ignore_urls: Vec<String>,

    // auth-basics
    #[serde(rename = "auth-basics")]
    pub auth_basics: Vec<String>,

    // auth
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub prefix: String,
    pub debug: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_size: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApalisConfig {
    pub url: String,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub expires_at: u64,
    pub refresh_expires_at: u64,
}
