use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub default_streams: usize,
    pub default_download_dir: String,
    pub auto_detect_clipboard: bool,
    pub auto_detect_browser: bool,
    pub interceptor_port: u16,
    pub user_agent: String,
    pub verify_sha256: bool,
    pub auto_warp: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Self {
            default_streams: 16,
            default_download_dir: download_dir,
            auto_detect_clipboard: true,
            auto_detect_browser: true,
            interceptor_port: 19800,
            user_agent: crate::resolver::DEFAULT_USER_AGENT.to_string(),
            verify_sha256: true,
            auto_warp: false,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        cwd.join("udm_config.json")
    }

    pub fn load() -> Self {
        let p = Self::config_path();
        if p.exists() {
            if let Ok(content) = fs::read_to_string(p) {
                if let Ok(cfg) = serde_json::from_str(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::config_path();
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(p, content).map_err(|e| e.to_string())
    }
}
