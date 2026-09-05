use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub application: AppConfig,
    pub database: DbConfig,
    pub secret: SecretConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuthConfig {
    /// Fixed OTP code for local development only. When set, `otp/request`
    /// issues this code instead of a random one. Must stay absent from
    /// production config — real delivery (SMS) lands with its own slice.
    #[serde(default)]
    pub dev_otp_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecretConfig {
    pub jwt_signing_key: SecretString,
}

// ---------------------------------------------------------------------------
// Object storage (slice U1) — images live in object storage, never in the
// database. Dev/test run on local disk and in memory; production is an
// S3-compatible bucket (Cloudflare R2). Clients only ever see URLs, so a
// backend swap is config, never code.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    #[default]
    Local,
    Memory,
    S3,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub backend: StorageBackend,
    /// Base URL every stored object is served from. Empty = the API's own
    /// file route (relative `/api/v1/files/…` URLs); production points at
    /// the R2 public URL (Cloudflare CDN in front). Per-environment because
    /// a phone on the LAN needs the machine's IP, not localhost.
    #[serde(default)]
    pub public_base_url: String,
    #[serde(default)]
    pub local: LocalStorageConfig,
    #[serde(default)]
    pub s3: S3StorageConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Local,
            public_base_url: String::new(),
            local: LocalStorageConfig::default(),
            s3: S3StorageConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalStorageConfig {
    #[serde(default = "default_storage_root")]
    pub root: String,
}

impl Default for LocalStorageConfig {
    fn default() -> Self {
        Self {
            root: default_storage_root(),
        }
    }
}

fn default_storage_root() -> String {
    "./data/storage".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3StorageConfig {
    #[serde(default)]
    pub bucket: String,
    /// S3-compatible endpoint. R2: `https://<account>.r2.cloudflarestorage.com`.
    /// Empty = real AWS S3.
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_s3_region")]
    pub region: String,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: SecretString,
}

impl Default for S3StorageConfig {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            endpoint: String::new(),
            region: default_s3_region(),
            access_key_id: String::new(),
            secret_access_key: SecretString::new(String::new().into()),
        }
    }
}

fn default_s3_region() -> String {
    "auto".into()
}

// ---------------------------------------------------------------------------
// Road routing (build order #4, slice D2) — Google Directions, called by OUR
// server (the key never reaches a client), cached on the delivery at
// handoff. Until a key is configured the `none` backend answers honestly:
// no route — callers fall back to their own estimates. The backend swap is
// config, never code (the storage pattern).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingBackend {
    #[default]
    None,
    Google,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    #[serde(default)]
    pub backend: RoutingBackend,
    /// The Directions API key — server-only, configured when the map
    /// slices land. Absent/empty is valid while backend is `none`.
    #[serde(default)]
    pub api_key: Option<SecretString>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            backend: RoutingBackend::None,
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub cookie_secure: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    #[serde(default)]
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_secret")]
    pub password: SecretString,
    pub database_name: String,
    pub require_ssl: bool,
}

fn default_secret() -> SecretString {
    SecretString::new(String::new().into())
}

impl DbConfig {
    pub fn without_db(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(self.password.expose_secret())
            .ssl_mode(self.ssl_mode())
    }
    pub fn with_db(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(self.password.expose_secret())
            .database(&self.database_name)
            .ssl_mode(self.ssl_mode())
    }
    fn ssl_mode(&self) -> PgSslMode {
        if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        }
    }
}

impl Config {
    pub fn database_with_db(&self) -> PgConnectOptions {
        match std::env::var("DATABASE_URL") {
            Ok(url) if !url.is_empty() => PgConnectOptions::from_str(&url).expect(
                "DATABASE_URL was set but could not be parsed as a Postgres connection string",
            ),
            _ => {
                assert!(
                    !self.database.host.is_empty(),
                    "no DATABASE_URL set and database.host is empty — set DATABASE_URL or the APP_DATABASE__* variables"
                );
                self.database.with_db()
            }
        }
    }
}

pub fn get_configuration(config_dir: &Path) -> Result<Config, config::ConfigError> {
    let environment = Environment::current();

    let builder = config::Config::builder()
        .add_source(config::File::from(config_dir.join("base")).required(true))
        .add_source(config::File::from(config_dir.join(environment.to_string())).required(true))
        .add_source(
            config::Environment::with_prefix("app")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    let configuration = builder.try_deserialize::<Config>()?;
    Ok(configuration)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn current() -> Self {
        let _ = dotenvy::dotenv();
        std::env::var("APP_ENV")
            .unwrap_or_else(|_| "local".into())
            .try_into()
            .expect("Failed to parse APP_ENV.")
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let env_str = match self {
            Environment::Local => "local",
            Environment::Production => "production",
        };
        write!(f, "{}", env_str)
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.as_str().try_into()
    }
}
impl TryFrom<&str> for Environment {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
