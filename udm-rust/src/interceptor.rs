use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadPayload {
    pub url: String,
    pub filename: Option<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub cookies: Option<String>,
    pub auto_start: Option<bool>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AutoDetectConfig {
    pub enable_clipboard_monitoring: bool,
    pub enable_browser_interceptor: bool,
    pub interceptor_port: u16,
    pub auto_start_downloads: bool,
    pub monitored_extensions: HashSet<String>,
}

impl Default for AutoDetectConfig {
    fn default() -> Self {
        let mut extensions = HashSet::new();
        for ext in &[
            "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "img", "bin",
            "exe", "msi", "dmg", "pkg", "deb", "rpm", "apk",
            "mp4", "mkv", "avi", "mov", "webm", "mp3", "wav", "flac",
            "pdf", "epub", "csv", "torrent",
        ] {
            extensions.insert(ext.to_string());
        }

        Self {
            enable_clipboard_monitoring: true,
            enable_browser_interceptor: true,
            interceptor_port: 19800,
            auto_start_downloads: true,
            monitored_extensions: extensions,
        }
    }
}

pub struct AutoDetectionEngine {
    config: AutoDetectConfig,
    tx_download: UnboundedSender<DownloadPayload>,
    is_running: Arc<AtomicBool>,
}

impl AutoDetectionEngine {
    pub fn new(config: AutoDetectConfig, tx_download: UnboundedSender<DownloadPayload>) -> Self {
        Self {
            config,
            tx_download,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_downloadable_url(&self, text: &str) -> bool {
        let trimmed = text.trim();
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return false;
        }

        // Special handling for popular sharing platforms
        if trimmed.contains("mediafire.com/file/") || trimmed.contains("mediafire.com/download/") {
            return true;
        }
        if trimmed.contains("drive.google.com/file/") || trimmed.contains("drive.google.com/open?") {
            return true;
        }
        if trimmed.contains("github.com/") && trimmed.contains("/releases/download/") {
            return true;
        }

        // Check if URL ends with recognized extension
        if let Ok(parsed) = url::Url::parse(trimmed) {
            let path = parsed.path().to_lowercase();
            for ext in &self.config.monitored_extensions {
                if path.ends_with(&format!(".{}", ext)) {
                    return true;
                }
            }
        }

        false
    }

    /// Launches the browser interceptor HTTP daemon (listening for browser extension hooks)
    pub async fn start_browser_interceptor(&self) -> Result<(), String> {
        let port = self.config.interceptor_port;
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.map_err(|e| format!("Failed to bind interceptor port {}: {}", port, e))?;
        println!("[Auto-Detect] Browser Interception Daemon active on http://{}", addr);

        let tx = self.tx_download.clone();
        let running = self.is_running.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let tx_conn = tx.clone();
                    tokio::spawn(async move {
                        let mut buffer = [0u8; 8192];
                        let n = match socket.read(&mut buffer).await {
                            Ok(n) if n > 0 => n,
                            _ => return,
                        };

                        let req_text = String::from_utf8_lossy(&buffer[..n]);

                        // Handle CORS preflight
                        if req_text.starts_with("OPTIONS") {
                            let cors_resp = "HTTP/1.1 204 No Content\r\n\
                                            Access-Control-Allow-Origin: *\r\n\
                                            Access-Control-Allow-Methods: POST, GET, OPTIONS\r\n\
                                            Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
                                            Connection: close\r\n\r\n";
                            let _ = socket.write_all(cors_resp.as_bytes()).await;
                            return;
                        }

                        // Handle POST /api/download
                        if req_text.contains("POST /api/download") || req_text.contains("POST /jsonrpc") {
                            if let Some(body_start) = req_text.find("\r\n\r\n") {
                                let body = &req_text[body_start + 4..];
                                if let Ok(payload) = serde_json::from_str::<DownloadPayload>(body) {
                                    println!("\n[Browser Auto-Detect] Intercepted incoming download from browser/app: {}", payload.url);
                                    let _ = tx_conn.send(payload);

                                    let resp_json = r#"{"status":"success","message":"Download captured and queued by UDM Rust"}"#;
                                    let resp = format!(
                                        "HTTP/1.1 200 OK\r\n\
                                        Content-Type: application/json\r\n\
                                        Access-Control-Allow-Origin: *\r\n\
                                        Content-Length: {}\r\n\
                                        Connection: close\r\n\r\n{}",
                                        resp_json.len(),
                                        resp_json
                                    );
                                    let _ = socket.write_all(resp.as_bytes()).await;
                                    return;
                                }
                            }
                        }

                        // Fallback response for GET /
                        let resp_body = r#"{"udm_status":"active","engine":"Rust","version":"1.0.0"}"#;
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\n\
                            Content-Type: application/json\r\n\
                            Access-Control-Allow-Origin: *\r\n\
                            Content-Length: {}\r\n\
                            Connection: close\r\n\r\n{}",
                            resp_body.len(),
                            resp_body
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    });
                }
            }
        });

        Ok(())
    }

    /// Starts system clipboard watcher for non-browser desktop applications (Discord, Slack, Telegram, PDF readers, etc.)
    pub async fn start_clipboard_monitor(&self) {
        let tx = self.tx_download.clone();
        let running = self.is_running.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            println!("[Auto-Detect] System Clipboard Auto-Detection Monitor active (checking copied URLs)...");
            let mut last_clipboard = String::new();

            while running.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(800)).await;

                // Read clipboard using native OS command
                let current_clip = Self::read_system_clipboard();
                if let Some(text) = current_clip {
                    let trimmed = text.trim().to_string();
                    if trimmed != last_clipboard {
                        last_clipboard = trimmed.clone();

                        let eng = AutoDetectionEngine::new(config.clone(), tx.clone());
                        if eng.is_downloadable_url(&trimmed) {
                            println!("\n[Clipboard Auto-Detect] Detected downloadable link on clipboard: {}", trimmed);
                            let payload = DownloadPayload {
                                url: trimmed,
                                filename: None,
                                referer: None,
                                user_agent: None,
                                cookies: None,
                                auto_start: Some(config.auto_start_downloads),
                                source: Some("System Clipboard".to_string()),
                            };
                            let _ = tx.send(payload);
                        }
                    }
                }
            }
        });
    }

    pub fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    fn read_system_clipboard() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", "Get-Clipboard"])
                .output()
                .ok()?;
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("pbpaste").output().ok()?;
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("xclip").args(["-selection", "clipboard", "-o"]).output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }
}
