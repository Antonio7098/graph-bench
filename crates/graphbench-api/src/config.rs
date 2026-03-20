use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_fixtures_dir")]
    pub fixtures_dir: PathBuf,
    #[serde(default)]
    pub database_path: Option<PathBuf>,
    #[serde(default = "default_openrouter_app_title")]
    pub openrouter_app_title: String,
    #[serde(default = "default_openrouter_referer")]
    pub openrouter_referer: String,
    #[serde(default)]
    pub openrouter_api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenRouterSettings {
    pub api_key: Option<String>,
    pub app_title: String,
    pub referer: String,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("graphbench.config.toml"));

        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file {}", path.display()))?;

            let cfg: Config = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config file {}", path.display()))?;
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }
}

impl ApiConfig {
    pub fn database_path(&self) -> PathBuf {
        self.database_path
            .clone()
            .unwrap_or_else(|| self.data_dir.join("graphbench.db"))
    }

    pub fn openrouter_settings(&self, env_api_key: Option<String>) -> OpenRouterSettings {
        OpenRouterSettings {
            api_key: env_api_key.or_else(|| self.openrouter_api_key.clone()),
            app_title: self.openrouter_app_title.clone(),
            referer: self.openrouter_referer.clone(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            data_dir: default_data_dir(),
            fixtures_dir: default_fixtures_dir(),
            database_path: None,
            openrouter_app_title: default_openrouter_app_title(),
            openrouter_referer: default_openrouter_referer(),
            openrouter_api_key: None,
        }
    }
}

fn default_port() -> u16 {
    3001
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_fixtures_dir() -> PathBuf {
    PathBuf::from("./fixtures")
}

fn default_openrouter_app_title() -> String {
    "GraphBench".to_string()
}

fn default_openrouter_referer() -> String {
    "http://localhost:5173".to_string()
}
