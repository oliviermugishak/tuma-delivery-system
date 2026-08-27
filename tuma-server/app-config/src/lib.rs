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
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecretConfig {
    pub jwt_signing_key: SecretString,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub cookie_secure: bool,
    #[serde(default)]
    pub static_dir: Option<String>,
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
