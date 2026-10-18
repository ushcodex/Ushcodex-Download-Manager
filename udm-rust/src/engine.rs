use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, RANGE, USER_AGENT};
use tokio::sync::mpsc::UnboundedSender;

use crate::resolver::{DEFAULT_USER_AGENT, ResolvedStream};

pub const DEFAULT_STREAMS: usize = 16;
pub const BUFFER_SIZE: usize = 128 * 1024; // 128 KB chunk buffer

pub fn human_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, units[unit_idx])
}

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
    pub active_streams: usize,
    pub total_streams: usize,
    pub percentage: f64,
    pub eta_seconds: u64,
}

pub struct DownloadEngine {
    client: reqwest::Client,
    cancel_flag: Arc<AtomicBool>,
}

impl DownloadEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .pool_max_idle_per_host(32)
            .build()
            .unwrap_or_default();

        Self {
            client,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    pub async fn download(
        &self,
        stream_info: &ResolvedStream,
        output_path: &Path,
        streams_count: usize,
        progress_tx: Option<UnboundedSender<ProgressEvent>>,
    ) -> Result<PathBuf, String> {
        let final_dest = output_path.to_path_buf();
        let total_size = stream_info.total_size;

        if !stream_info.supports_ranges || total_size == 0 {
            // Fallback: single optimized stream
            println!("[Engine] Server does not support Range requests. Downloading via single optimized stream...");
            return self.download_single_stream(stream_info, &final_dest, progress_tx).await;
        }

        let streams = streams_count.clamp(1, 64);
        let chunk_size = (total_size as f64 / streams as f64).ceil() as u64;

        let parts_dir = PathBuf::from(format!("{}.udm_parts", final_dest.display()));
        fs::create_dir_all(&parts_dir).map_err(|e| format!("Failed to create parts dir: {}", e))?;

        let downloaded_bytes_counter = Arc::new(AtomicU64::new(0));
        let mut initial_downloaded = 0u64;

        // 1. Inspect existing files on disk for true resume
        for i in 0..streams {
            let part_path = parts_dir.join(format!("part_{:03}.tmp", i));
            if part_path.exists() {
                if let Ok(meta) = fs::metadata(&part_path) {
                    let cur_len = meta.len();
                    let start = i as u64 * chunk_size;
                    let end = ((i as u64 + 1) * chunk_size - 1).min(total_size - 1);
                    let expected_chunk = (end - start) + 1;

                    let valid_len = cur_len.min(expected_chunk);
                    initial_downloaded += valid_len;
                }
            }
        }

        downloaded_bytes_counter.store(initial_downloaded, Ordering::SeqCst);

        let mut tasks = Vec::new();

        println!(
            "[Engine] Starting multi-stream download: {} across {} parallel connections",
            human_bytes(total_size),
            streams
        );

        for i in 0..streams {
            let start = i as u64 * chunk_size;
            let end = ((i as u64 + 1) * chunk_size - 1).min(total_size - 1);
            let part_path = parts_dir.join(format!("part_{:03}.tmp", i));
            let client = self.client.clone();
            let url = stream_info.direct_url.clone();
            let counter = downloaded_bytes_counter.clone();
            let cancel = self.cancel_flag.clone();

            let handle = tokio::spawn(async move {
                Self::worker_chunk(i, url, client, start, end, part_path, counter, cancel).await
            });

            tasks.push(handle);
        }

        // 2. Monitoring & Progress loop
        let counter_reader = downloaded_bytes_counter.clone();
        let cancel_monitor = self.cancel_flag.clone();
        let progress_channel = progress_tx.clone();

        let monitor_handle = tokio::spawn(async move {
            let mut last_bytes = counter_reader.load(Ordering::SeqCst);
            let mut last_instant = Instant::now();

            while !cancel_monitor.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let cur_bytes = counter_reader.load(Ordering::SeqCst);
                let now = Instant::now();
                let elapsed = now.duration_since(last_instant).as_secs_f64();

                if elapsed >= 0.5 {
                    let diff = cur_bytes.saturating_sub(last_bytes);
                    let speed = diff as f64 / elapsed;
                    last_bytes = cur_bytes;
                    last_instant = now;

                    let pct = if total_size > 0 {
                        (cur_bytes as f64 / total_size as f64) * 100.0
                    } else {
                        0.0
                    };

                    let remaining = total_size.saturating_sub(cur_bytes);
                    let eta = if speed > 1024.0 {
                        (remaining as f64 / speed) as u64
                    } else {
                        0
                    };

                    if let Some(ref tx) = progress_channel {
                        let _ = tx.send(ProgressEvent {
                            downloaded_bytes: cur_bytes,
                            total_bytes: total_size,
                            speed_bytes_per_sec: speed,
                            active_streams: streams,
                            total_streams: streams,
                            percentage: pct,
                            eta_seconds: eta,
                        });
                    }

                    let bar_len = 25;
                    let filled = ((pct / 100.0) * bar_len as f64) as usize;
                    let bar = format!(
                        "[{}{}]",
                        "=".repeat(filled),
                        " ".repeat(bar_len.saturating_sub(filled))
                    );

                    let eta_str = if eta > 0 {
                        format!("{:02}:{:02}", eta / 60, eta % 60)
                    } else {
                        "--:--".to_string()
                    };

                    print!(
                        "\r{} {:5.1}% | {}/{} | {}/s | ETA: {} | Streams: {}",
                        bar,
                        pct,
                        human_bytes(cur_bytes),
                        human_bytes(total_size),
                        human_bytes(speed as u64),
                        eta_str,
                        streams
                    );
                    let _ = std::io::stdout().flush();

                    if cur_bytes >= total_size {
                        break;
                    }
                }
            }
        });

        // 3. Wait for all chunk workers to complete
        for t in tasks {
            match t.await {
                Ok(res) => res?,
                Err(e) => return Err(format!("Task execution panicked: {}", e)),
            }
        }

        monitor_handle.abort();
        println!("\n[Engine] All chunk streams completed 100%! Verifying and assembling...");

        // 4. Verification and Atomic Chunk Assembly
        Self::assemble_chunks(&parts_dir, &final_dest, streams, total_size, chunk_size)?;

        Ok(final_dest)
    }

    async fn worker_chunk(
        _id: usize,
        url: String,
        client: reqwest::Client,
        start_b: u64,
        end_b: u64,
        part_path: PathBuf,
        counter: Arc<AtomicU64>,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let total_chunk_len = (end_b - start_b) + 1;
        let mut existing_len = 0u64;

        if part_path.exists() {
            if let Ok(meta) = fs::metadata(&part_path) {
                existing_len = meta.len();
                if existing_len > total_chunk_len {
                    // Truncate if corrupted by unexpected crash
                    let f = OpenOptions::new().write(true).open(&part_path).map_err(|e| e.to_string())?;
                    f.set_len(total_chunk_len).map_err(|e| e.to_string())?;
                    existing_len = total_chunk_len;
                }
            }
        }

        if existing_len >= total_chunk_len {
            return Ok(());
        }

        let mut current_start = start_b + existing_len;
        let mut retries = 0;

        while current_start <= end_b {
            if cancel.load(Ordering::SeqCst) {
                return Err("Download paused or stopped by user".to_string());
            }

            let range_header = format!("bytes={}-{}", current_start, end_b);
            let req = client.get(&url).header(RANGE, range_header);

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() && status.as_u16() != 206 {
                        retries += 1;
                        if retries > 10 {
                            return Err(format!("Worker failed: HTTP {}", status));
                        }
                        tokio::time::sleep(Duration::from_millis(1500)).await;
                        continue;
                    }

                    let mut file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(&part_path)
                        .map_err(|e| format!("Failed to open part file: {}", e))?;

                    let mut stream = resp.bytes_stream();

                    while let Some(chunk_res) = stream.next().await {
                        if cancel.load(Ordering::SeqCst) {
                            return Err("Download interrupted".to_string());
                        }

                        let chunk = chunk_res.map_err(|e| format!("Stream error: {}", e))?;
                        let chunk_len = chunk.len() as u64;

                        // Ensure we don't exceed chunk boundary
                        let remaining = total_chunk_len.saturating_sub(existing_len);
                        let to_write = (chunk_len).min(remaining);

                        file.write_all(&chunk[..to_write as usize])
                            .map_err(|e| format!("Disk write error: {}", e))?;

                        existing_len += to_write;
                        current_start += to_write;
                        counter.fetch_add(to_write, Ordering::SeqCst);

                        if existing_len >= total_chunk_len {
                            break;
                        }
                    }

                    if existing_len >= total_chunk_len {
                        return Ok(());
                    }
                }
                Err(_) => {
                    retries += 1;
                    if retries > 10 {
                        return Err("Max connection retries exceeded on chunk worker".to_string());
                    }
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        }

        Ok(())
    }

    fn assemble_chunks(
        parts_dir: &Path,
        output_file: &Path,
        streams: usize,
        total_size: u64,
        chunk_size: u64,
    ) -> Result<(), String> {
        // Validate each chunk's exact byte boundary before merging
        for i in 0..streams {
            let part_path = parts_dir.join(format!("part_{:03}.tmp", i));
            if !part_path.exists() {
                return Err(format!("Chunk part_{:03}.tmp missing", i));
            }

            let start = i as u64 * chunk_size;
            let end = ((i as u64 + 1) * chunk_size - 1).min(total_size - 1);
            let expected_len = (end - start) + 1;

            let meta = fs::metadata(&part_path).map_err(|e| e.to_string())?;
            if meta.len() != expected_len {
                return Err(format!(
                    "Chunk {} corrupted: expected {} bytes, got {}",
                    i, expected_len, meta.len()
                ));
            }
        }

        let mut out_file = File::create(output_file).map_err(|e| format!("Cannot create final file: {}", e))?;
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB assembly buffer

        for i in 0..streams {
            let part_path = parts_dir.join(format!("part_{:03}.tmp", i));
            let mut part_f = File::open(&part_path).map_err(|e| e.to_string())?;

            loop {
                let n = part_f.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                out_file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            }

            let _ = fs::remove_file(part_path);
        }

        let _ = fs::remove_dir(parts_dir);

        let final_size = fs::metadata(output_file).map_err(|e| e.to_string())?.len();
        if final_size != total_size {
            return Err(format!(
                "Assembly byte check failed: expected {} bytes, got {}",
                total_size, final_size
            ));
        }

        Ok(())
    }

    async fn download_single_stream(
        &self,
        stream_info: &ResolvedStream,
        output_path: &Path,
        progress_tx: Option<UnboundedSender<ProgressEvent>>,
    ) -> Result<PathBuf, String> {
        let resp = self
            .client
            .get(&stream_info.direct_url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let total_size = stream_info.total_size;
        let mut file = File::create(output_path).map_err(|e| format!("Could not create file: {}", e))?;
        let mut stream = resp.bytes_stream();
        let mut downloaded = 0u64;
        let start_time = Instant::now();
        let mut last_instant = Instant::now();
        let mut last_bytes = 0u64;

        while let Some(chunk_res) = stream.next().await {
            if self.is_cancelled() {
                return Err("Download cancelled".to_string());
            }

            let chunk = chunk_res.map_err(|e| format!("Read chunk error: {}", e))?;
            file.write_all(&chunk).map_err(|e| format!("Write error: {}", e))?;
            downloaded += chunk.len() as u64;

            let now = Instant::now();
            let elapsed = now.duration_since(last_instant).as_secs_f64();
            if elapsed >= 0.5 {
                let speed = (downloaded - last_bytes) as f64 / elapsed;
                last_bytes = downloaded;
                last_instant = now;

                let pct = if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                };

                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(ProgressEvent {
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: speed,
                        active_streams: 1,
                        total_streams: 1,
                        percentage: pct,
                        eta_seconds: 0,
                    });
                }
            }
        }

        Ok(output_path.to_path_buf())
    }
}
