use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub projects: Vec<Project>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub connection: ConnectionConfig,
    pub saved_requests: Vec<SavedRequest>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum ConnectionConfig {
    Reflection { url: String },
    File { url: String, path: PathBuf },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedRequest {
    pub id: Uuid,
    pub name: String,
    pub service: String,
    pub method: String,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "granc", "granc-tui")
            .context("Could not determine config directory")?;
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir)?;

        Ok(Self {
            config_path: config_dir.join("config.json"),
        })
    }

    pub fn load(&self) -> Result<AppConfig> {
        if !self.config_path.exists() {
            return Ok(AppConfig::default());
        }
        let content = fs::read_to_string(&self.config_path)?;
        let config = serde_json::from_str(&content).unwrap_or_default();
        Ok(config)
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }
}
