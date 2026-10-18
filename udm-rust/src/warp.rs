use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WarpManager;

impl WarpManager {
    pub fn find_warp_cli() -> Option<PathBuf> {
        let standard_paths = [
            r"C:\Program Files\Cloudflare\Cloudflare WARP\warp-cli.exe",
            r"C:\Program Files (x86)\Cloudflare\Cloudflare WARP\warp-cli.exe",
            "/usr/bin/warp-cli",
            "/usr/local/bin/warp-cli",
        ];

        for p in standard_paths {
            let path = Path::new(p);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }

        // Try PATH search
        if let Ok(path) = which::which("warp-cli") {
            return Some(path);
        }

        None
    }

    pub fn is_installed() -> bool {
        Self::find_warp_cli().is_some()
    }

    pub fn get_status() -> (String, String) {
        let cli = match Self::find_warp_cli() {
            Some(c) => c,
            None => return ("Not Installed".to_string(), "Cloudflare WARP client is not detected.".to_string()),
        };

        match Command::new(cli).arg("status").output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
                let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
                let combined = format!("{} {}", stdout, stderr);

                if combined.contains("connected") && !combined.contains("disconnected") {
                    ("Connected".to_string(), "Active: Traffic is tunneled through Cloudflare WireGuard UDP (Bypassing Throttle)".to_string())
                } else if combined.contains("connecting") {
                    ("Connecting".to_string(), "Connecting to Cloudflare edge tunnel...".to_string())
                } else if combined.contains("disconnected") {
                    ("Disconnected".to_string(), "WARP is installed but currently disconnected.".to_string())
                } else {
                    ("Ready".to_string(), stdout.trim().to_string())
                }
            }
            Err(e) => ("Error".to_string(), format!("Failed to query warp-cli: {}", e)),
        }
    }

    pub fn connect() -> Result<String, String> {
        let cli = match Self::find_warp_cli() {
            Some(c) => c,
            None => return Err("WARP is not installed".to_string()),
        };

        // Try registering if needed
        let _ = Command::new(&cli).args(["registration", "new"]).output();

        match Command::new(&cli).arg("connect").output() {
            Ok(_) => Ok("Cloudflare WARP UDP tunnel enabled successfully.".to_string()),
            Err(e) => Err(format!("Failed to connect WARP: {}", e)),
        }
    }

    pub fn disconnect() -> Result<String, String> {
        let cli = match Self::find_warp_cli() {
            Some(c) => c,
            None => return Err("WARP is not installed".to_string()),
        };

        match Command::new(&cli).arg("disconnect").output() {
            Ok(_) => Ok("WARP disconnected. Standard network routing restored.".to_string()),
            Err(e) => Err(format!("Failed to disconnect WARP: {}", e)),
        }
    }
}
