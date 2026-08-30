pub use app_config::{AppConfig, Config, DbConfig, Environment, StorageBackend, StorageConfig};
use std::path::PathBuf;

pub fn get_configuration() -> Result<Config, config::ConfigError> {
    let config_dir =
        std::env::var("APP_CONFIGURATION_DIR").unwrap_or_else(|_| "./configuration".to_owned());
    let config_dir = PathBuf::from(config_dir);
    app_config::get_configuration(&config_dir)
}
