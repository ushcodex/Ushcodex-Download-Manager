use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadRecord {
    pub id: String,
    pub filename: String,
    pub original_url: String,
    pub direct_url: String,
    pub output_path: String,
    pub file_size: u64,
    pub formatted_size: String,
    pub status: String,
    pub streams_used: usize,
    pub sha256_hash: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

pub struct HistoryManager {
    file_path: PathBuf,
}

impl HistoryManager {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let file_path = base_dir.as_ref().join("history.json");
        Self { file_path }
    }

    pub fn default_instance() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(cwd)
    }

    pub fn load_history(&self) -> Vec<DownloadRecord> {
        if !self.file_path.exists() {
            return Vec::new();
        }
        match fs::read_to_string(&self.file_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn save_history(&self, records: &[DownloadRecord]) -> io::Result<()> {
        let json = serde_json::to_string_pretty(records)?;
        fs::write(&self.file_path, json)
    }

    pub fn add_or_update(&self, record: DownloadRecord) -> io::Result<()> {
        let mut list = self.load_history();
        if let Some(pos) = list.iter().position(|r| r.id == record.id) {
            list[pos] = record;
        } else {
            list.insert(0, record);
        }
        self.save_history(&list)
    }

    pub fn compute_sha256<P: AsRef<Path>>(path: P) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 1024 * 512]; // 512KB buffer

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash_bytes = hasher.finalize();
        Ok(format!("{:x}", hash_bytes))
    }
}
