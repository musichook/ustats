use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Read Claude Code's OAuth access token from the macOS Keychain.
/// Claude Code stores credentials under service "Claude Code-credentials".
fn read_claude_code_token() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json_str = String::from_utf8(output.stdout).ok()?.trim().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let token = parsed
        .get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()?
        .to_string();

    if token.is_empty() {
        return None;
    }

    Some(token)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub polling: PollingConfig,
    #[serde(default)]
    pub widget: WidgetConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    #[serde(default = "default_true")]
    pub show_on_launch: bool,
    #[serde(default = "default_x")]
    pub position_x: f64,
    #[serde(default = "default_y")]
    pub position_y: f64,
}

fn default_interval() -> u64 { 60 }
fn default_true() -> bool { true }
fn default_x() -> f64 { 100.0 }
fn default_y() -> f64 { 100.0 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig { api_key: String::new() },
            polling: PollingConfig { interval_seconds: 60 },
            widget: WidgetConfig { show_on_launch: true, position_x: 100.0, position_y: 100.0 },
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self { Self { api_key: String::new() } }
}

impl Default for PollingConfig {
    fn default() -> Self { Self { interval_seconds: 60 } }
}

impl Default for WidgetConfig {
    fn default() -> Self { Self { show_on_launch: true, position_x: 100.0, position_y: 100.0 } }
}

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("ustats").join("config.toml")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let contents = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, contents).map_err(|e| e.to_string())
    }

    /// Returns the API key with priority:
    /// 1. Claude Code OAuth token from macOS Keychain
    /// 2. ANTHROPIC_API_KEY env var
    /// 3. Manually configured key in config file
    pub fn api_key(&self) -> Option<String> {
        // Try Claude Code's OAuth token from macOS Keychain first
        if let Some(token) = read_claude_code_token() {
            return Some(token);
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                return Some(key);
            }
        }
        if !self.auth.api_key.is_empty() {
            return Some(self.auth.api_key.clone());
        }
        None
    }
}
