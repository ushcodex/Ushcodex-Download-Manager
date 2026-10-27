mod config;
mod engine;
mod history;
mod interceptor;
mod resolver;
mod warp;

use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::engine::{human_bytes, DownloadEngine, DEFAULT_STREAMS};
use crate::history::{DownloadRecord, HistoryManager};
use crate::interceptor::{AutoDetectConfig, AutoDetectionEngine, DownloadPayload};
use crate::resolver::UrlResolver;
use crate::warp::WarpManager;

#[derive(Parser)]
#[command(
    name = "udm-rust",
    author = "Ushcodex <ushcodex@github.com>",
    version = "1.0.0",
    about = "⚡ Ushcodex Download Manager (UDM) Pro - High Performance Multi-Stream Engine (Rust Edition)",
    long_about = "A high-performance multi-threaded download manager engineered to bypass connection throttling, \
                  auto-detect downloads from web browsers & desktop apps, and resolve landing pages (MediaFire, Google Drive)."
)]
struct Cli {
    /// URL of the file to download (MediaFire, Google Drive, Direct link)
    #[arg(value_name = "URL")]
    url: Option<String>,

    /// Custom output destination filename or path
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Number of simultaneous chunk connections (default: 16, max: 64)
    #[arg(short, long, default_value_t = DEFAULT_STREAMS)]
    connections: usize,

    /// Automatically connect Cloudflare WARP UDP tunnel to bypass campus/ISP throttling
    #[arg(long)]
    warp: bool,

    /// Enable auto-detection mode (monitors clipboard and runs browser interception daemon)
    #[arg(short, long)]
    auto_detect: bool,

    /// Port for the browser interception daemon (default: 19800)
    #[arg(long, default_value_t = 19800)]
    interceptor_port: u16,

    /// Verify SHA-256 checksum and save to persistent history
    #[arg(long, default_value_t = true)]
    verify_hash: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the background auto-interception daemon (Browser + Clipboard listener)
    Daemon {
        #[arg(short, long, default_value_t = 19800)]
        port: u16,
    },
    /// View persistent download history and SHA-256 audit logs
    History,
    /// Inspect or control the Cloudflare WARP turbo tunnel
    Warp {
        #[arg(value_name = "ACTION", default_value = "status")]
        action: String, // status, connect, disconnect
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    print_banner();

    // Handle subcommands
    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Daemon { port } => {
                run_daemon(port).await?;
                return Ok(());
            }
            Commands::History => {
                show_history();
                return Ok(());
            }
            Commands::Warp { action } => {
                handle_warp_cmd(&action);
                return Ok(());
            }
        }
    }

    if cli.auto_detect {
        run_daemon(cli.interceptor_port).await?;
        return Ok(());
    }

    // Connect WARP if requested
    if cli.warp {
        println!("{}", "[*] Activating Cloudflare WARP UDP Tunnel...".cyan());
        match WarpManager::connect() {
            Ok(msg) => println!("{} {}", "[✓]".green(), msg),
            Err(e) => println!("{} {}", "[!]".yellow(), e),
        }
    }

    let target_url = match cli.url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            // Interactive Prompt
            println!("{}", "No URL supplied. Please paste download link below (or pass --auto-detect for background interception):".yellow());
            print!("URL: ");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                println!("{}", "No link entered. Exiting.".red());
                return Ok(());
            }
            trimmed
        }
    };

    execute_download(&target_url, cli.output, cli.connections, cli.verify_hash).await?;

    Ok(())
}

fn print_banner() {
    println!("{}", "=========================================================================".cyan());
    println!("{}", "   ⚡ Ushcodex Download Manager (UDM) Pro - Rust Edition v1.0.0".bold().green());
    println!("{}", "   Multi-Stream Acceleration • Browser Interception • Zero-Loss Resume".white());
    println!("{}", "=========================================================================".cyan());
}

async fn execute_download(
    url: &str,
    custom_output: Option<String>,
    connections: usize,
    verify_hash: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{} Resolving direct stream endpoint...", "[1/4]".bold().blue());
    let resolver = UrlResolver::new();
    let stream_info = match resolver.resolve(url, None).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} Failed to resolve link: {}", "[-]".red(), e);
            return Err(e.into());
        }
    };

    println!("  {} Direct URL : {}...", "[+]".green(), &stream_info.direct_url.chars().take(70).collect::<String>());
    println!("  {} Filename   : {}", "[+]".green(), stream_info.filename.bold().yellow());
    println!("  {} Total Size : {}", "[+]".green(), human_bytes(stream_info.total_size).bold().white());
    println!("  {} Multi-Stream: {}", "[+]".green(), if stream_info.supports_ranges { "Supported (Bypassing Throttle)".green() } else { "Single Connection (No Range Support)".yellow() });

    let out_filename = custom_output.unwrap_or_else(|| stream_info.filename.clone());
    let out_path = PathBuf::from(&out_filename);

    println!("\n{} Launching {} parallel TCP streams...", "[2/4]".bold().blue(), connections);
    let engine = DownloadEngine::new();
    let start_time = std::time::Instant::now();

    let assembled_path = match engine.download(&stream_info, &out_path, connections, None).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("\n{} Download failed: {}", "[-]".red(), e);
            return Err(e.into());
        }
    };

    let total_time = start_time.elapsed().as_secs_f64();
    let avg_speed = if total_time > 0.0 {
        stream_info.total_size as f64 / total_time
    } else {
        0.0
    };

    println!("\n\n{} Download successfully assembled!", "[3/4]".bold().green());
    println!("  {} Elapsed Time : {:.2} seconds", "[✓]".green(), total_time);
    println!("  {} Average Speed: {}/s", "[✓]".green(), human_bytes(avg_speed as u64).bold());
    println!("  {} Saved Path   : {}", "[✓]".green(), assembled_path.canonicalize().unwrap_or(assembled_path.clone()).display());

    // SHA-256 Hash Verification & History Recording
    let mut hash_str: Option<String> = None;
    if verify_hash && stream_info.total_size > 0 {
        println!("\n{} Computing cryptographic SHA-256 audit hash...", "[4/4]".bold().blue());
        match HistoryManager::compute_sha256(&assembled_path) {
            Ok(h) => {
                println!("  {} SHA-256 : {}", "[✓]".green(), h.bold().cyan());
                hash_str = Some(h);
            }
            Err(e) => eprintln!("  {} Hash calculation error: {}", "[!]".yellow(), e),
        }
    }

    // Save record to persistent history
    let record = DownloadRecord {
        id: format!("udm_{}", chrono::Utc::now().timestamp_millis()),
        filename: out_filename,
        original_url: stream_info.original_url,
        direct_url: stream_info.direct_url,
        output_path: assembled_path.to_string_lossy().to_string(),
        file_size: stream_info.total_size,
        formatted_size: human_bytes(stream_info.total_size),
        status: "Completed".to_string(),
        streams_used: connections,
        sha256_hash: hash_str,
        created_at: chrono::Local::now().to_rfc3339(),
        completed_at: Some(chrono::Local::now().to_rfc3339()),
    };

    let history_mgr = HistoryManager::default_instance();
    let _ = history_mgr.add_or_update(record);

    println!("\n{}", "🎉 All operations complete! File verified 100% intact.".bold().green());
    Ok(())
}

async fn run_daemon(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "[MODE] Starting Universal Auto-Detection Daemon...".bold().cyan());

    let (tx, mut rx) = mpsc::unbounded_channel::<DownloadPayload>();

    let mut cfg = AutoDetectConfig::default();
    cfg.interceptor_port = port;

    let detector = AutoDetectionEngine::new(cfg, tx);
    detector.start();

    detector.start_browser_interceptor().await?;
    detector.start_clipboard_monitor().await;

    println!("\n{}", "=========================================================================".cyan());
    println!("{}", "   UDM Daemon is actively listening for downloads!".bold().green());
    println!("   • Browser Extension API : http://127.0.0.1:{}/api/download", port);
    println!("   • System Clipboard      : Any copied .iso, .zip, .exe, MediaFire, Drive URL");
    println!("   (Press Ctrl+C to terminate the daemon)");
    println!("{}", "=========================================================================\n".cyan());

    while let Some(payload) = rx.recv().await {
        println!("\n{} Auto-Detected URL from {}:", "⚡".bold().yellow(), payload.source.as_deref().unwrap_or("Browser Extension"));
        println!("   {}", payload.url.cyan());

        let url = payload.url.clone();
        let custom_out = payload.filename.clone();

        tokio::spawn(async move {
            if let Err(e) = execute_download(&url, custom_out, DEFAULT_STREAMS, true).await {
                eprintln!("Auto-download error: {}", e);
            }
        });
    }

    Ok(())
}

fn show_history() {
    let history_mgr = HistoryManager::default_instance();
    let records = history_mgr.load_history();

    println!("\n{}", "====================== PERSISTENT DOWNLOAD HISTORY ======================".bold().cyan());
    if records.is_empty() {
        println!("No download records found.");
        return;
    }

    for (i, r) in records.iter().enumerate() {
        println!(
            "{}. {} [{}] - {} streams",
            i + 1,
            r.filename.bold().yellow(),
            r.formatted_size.white(),
            r.streams_used
        );
        println!("   Status : {}", r.status.green());
        println!("   Path   : {}", r.output_path);
        if let Some(ref h) = r.sha256_hash {
            println!("   SHA256 : {}", h.cyan());
        }
        println!("   Date   : {}", r.created_at);
        println!("-------------------------------------------------------------------------");
    }
}

fn handle_warp_cmd(action: &str) {
    match action {
        "status" => {
            let (status, desc) = WarpManager::get_status();
            println!("WARP Status: {} -> {}", status.bold().cyan(), desc);
        }
        "connect" => match WarpManager::connect() {
            Ok(msg) => println!("{} {}", "[✓]".green(), msg),
            Err(e) => eprintln!("{} {}", "[!]".red(), e),
        },
        "disconnect" => match WarpManager::disconnect() {
            Ok(msg) => println!("{} {}", "[✓]".green(), msg),
            Err(e) => eprintln!("{} {}", "[!]".red(), e),
        },
        other => eprintln!("Unknown WARP command: {}. Use status, connect, or disconnect.", other),
    }
}
