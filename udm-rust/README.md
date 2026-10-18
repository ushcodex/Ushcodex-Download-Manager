# UDM Rust - Ushcodex Download Manager (Rust Edition) ⚡

> **High-Performance Multi-Stream Download Manager** — Rust rewrite of the original Python UDM, engineered for speed, cross-platform portability, and intelligent browser/app download interception.

---

## ✨ Key Features

| Feature | Description |
|:---|:---|
| 🚀 **Multi-Stream Segmented Downloading** | Splits files into 2–64 parallel HTTP Range streams, multiplying throughput across throttled campus/ISP links |
| 🌐 **Browser Auto-Interception** | Local HTTP daemon on port 19800 captures downloads from Chrome, Edge, Firefox, Brave — instantly |
| 📋 **Clipboard Auto-Detection** | Watches system clipboard for `.zip`, `.iso`, `.exe`, MediaFire, Google Drive links and auto-queues them |
| 🔗 **Smart URL Resolver** | Automatically resolves MediaFire `/file/` pages, Google Drive sharing links, and GitHub Releases into raw CDN streams |
| 🛡️ **Cloudflare WARP Turbo Mode** | One-command WireGuard UDP tunnel to bypass Layer-7 campus DPI throttles |
| ✅ **Crash-Proof Resume** | Atomic `.udm_parts/` chunk journal — never re-downloads completed bytes after power loss or network drop |
| 🔒 **SHA-256 Integrity Auditing** | Every completed download is cryptographically verified and logged to persistent history |
| 📜 **Persistent Download History** | Full JSON record of all downloads with timestamps, sizes, hashes, and stream counts |
| 🖥️ **CLI + Daemon + Interactive Mode** | Full-featured CLI, headless daemon, and interactive prompt mode |
| 🌍 **Cross-Platform Binaries** | Compiles for Windows (x86_64), Linux (x86_64 MUSL/GNU), macOS (Apple Silicon + Intel) |

---

## 🏗️ Project Structure

```
udm-rust/
├── src/
│   ├── main.rs          # CLI entry point, daemon mode, history display
│   ├── engine.rs        # Multi-stream async chunk download engine (Tokio)
│   ├── resolver.rs      # Smart URL resolver (MediaFire, Drive, headers)
│   ├── interceptor.rs   # Browser HTTP API + System Clipboard watcher
│   ├── history.rs       # Persistent download records + SHA-256 audit
│   ├── warp.rs          # Cloudflare WARP integration
│   └── config.rs        # JSON config management
├── browser-extension/
│   ├── manifest.json    # Chrome/Edge/Brave/Firefox Manifest V3
│   ├── background.js    # Download event interceptor service worker
│   ├── popup.html       # Extension popup UI
│   └── popup.js         # Daemon connection status + toggle
├── .cargo/
│   └── config.toml      # Build target: x86_64-pc-windows-gnu (MinGW linker)
├── Cargo.toml
├── Launch_UDM_Rust.bat      # 1-click Windows interactive launcher
└── build_all_platforms.bat  # Cross-platform multi-target build script
```

---

## 🚀 Quick Start

### Build (Windows)
```bat
# 1-click interactive menu:
Launch_UDM_Rust.bat

# Or build release binary directly:
cargo build --release
```

### Usage — Download a File
```bash
# Basic (auto-detects stream count, resolves landing pages)
udm-rust "https://www.mediafire.com/file/abc123/bigfile.iso/file"

# With custom connection count
udm-rust "https://example.com/file.zip" -c 32

# With Cloudflare WARP UDP tunnel for campus throttling bypass
udm-rust "https://example.com/file.zip" --warp -c 16
```

### Usage — Auto-Detection Daemon
```bash
# Start daemon — intercepts browser downloads AND clipboard URLs
udm-rust daemon --port 19800

# Or via batch file:
Launch_UDM_Rust.bat  →  Option [2]
```

### Usage — Browser Extension
1. Open Chrome/Edge → **Extensions** → **Load unpacked**
2. Select the `browser-extension/` folder
3. Make sure `udm-rust daemon` is running on port 19800
4. Any download in the browser is **instantly captured** by UDM!

### Usage — Subcommands
```bash
udm-rust history          # View all downloads with SHA-256 hashes
udm-rust warp status      # Check WARP tunnel status
udm-rust warp connect     # Enable WARP UDP bypass
udm-rust warp disconnect  # Restore normal routing
```

---

## 🌍 Cross-Platform Compilation

```bash
# Add cross-compile targets:
rustup target add x86_64-unknown-linux-musl   # Linux static binary
rustup target add aarch64-apple-darwin         # macOS Apple Silicon
rustup target add x86_64-apple-darwin          # macOS Intel

# Build for each:
cargo build --release --target x86_64-unknown-linux-musl
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Or run the automated pipeline:
build_all_platforms.bat
```

---

## 🔧 How Browser Auto-Detection Works

```
┌──────────────────────────────────────────────────────────┐
│             Browser (Chrome/Edge/Firefox)                │
│   User clicks a download link (.zip, .iso, MediaFire...) │
└────────────────────┬─────────────────────────────────────┘
                     │ Extension intercepts download.onCreated
                     │ Cancels native browser download
                     │ POSTs JSON payload to:
                     ▼
         http://127.0.0.1:19800/api/download
                     │
                     ▼
┌──────────────────────────────────────────────────────────┐
│              UDM Rust Daemon (background)                 │
│  - Resolves URL via SmartResolver (MediaFire/Drive/CDN)  │
│  - Launches 16+ parallel streams across throttle queues  │
│  - Writes atomically, resumes on reconnect               │
│  - SHA-256 verifies & logs to history.json               │
└──────────────────────────────────────────────────────────┘
```

---

## ⚙️ Competitive Advantages vs IDM / FDM / aria2

| | IDM | FDM | aria2 | **UDM Rust** |
|:---|:---:|:---:|:---:|:---:|
| Memory Usage | ~35 MB | ~150 MB | ~10 MB | **~15-25 MB** |
| Language | C/C++ | C++ | C++ | **Rust (Memory Safe)** |
| Browser Interception | ✅ (Extension) | ✅ | ⚠️ (JSON-RPC only) | ✅ **Native HTTP API + Clipboard** |
| Cloud Link Resolver | Basic | Basic | ❌ | ✅ **MediaFire, Google Drive, CDN** |
| WARP/VPN Bypass | ❌ | ❌ | ❌ | ✅ **Built-in Cloudflare WARP** |
| Windows SDK Required | ✅ | ✅ | ✅ | ❌ **GNU, zero SDK dependency** |
| Cross-Platform | Win only | Win/Mac/Linux | Win/Mac/Linux | ✅ **Win/Linux/macOS** |
| License | Paid | Freemium | MIT | **MIT (Open Source)** |

---

## License
MIT — Developed by Ushcodex
