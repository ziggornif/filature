use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub i18n: I18nConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct I18nConfig {
    pub default_locale: String,
}

impl Config {
    /// TOML file (if present) with FILATURE_ env overrides (double underscore = nesting,
    /// e.g. FILATURE_SERVER__BIND=0.0.0.0:9000).
    // `figment::Error` is inherently large (it carries rich error context); this runs
    // once at startup, not on a hot path, so boxing it would add ceremony for no gain.
    #[allow(clippy::result_large_err)]
    pub fn load(toml_path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(toml_path))
            .merge(Env::prefixed("FILATURE_").split("__"))
            .extract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::result_large_err)]
    fn env_overrides_toml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "filature.toml",
                r#"
                [server]
                bind = "127.0.0.1:8080"
                [database]
                url = "sqlite://filature.db"
                [i18n]
                default_locale = "fr"
                "#,
            )?;
            jail.set_env("FILATURE_SERVER__BIND", "0.0.0.0:9000");
            let cfg = Config::load("filature.toml").unwrap();
            assert_eq!(cfg.server.bind, "0.0.0.0:9000"); // env wins
            assert_eq!(cfg.i18n.default_locale, "fr"); // toml value kept
            Ok(())
        });
    }
}
