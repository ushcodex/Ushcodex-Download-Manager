# Ushcodex Download Manager (UDM) ⚡

A high-performance, multi-threaded desktop download manager engineered to bypass single-stream connection throttling, overcome institutional/campus QoS queues, and intelligently resolve file sharing landing pages (Mediafire, Google Drive, direct streams).

---

## Key Features

* **Smart URL & Landing Page Resolver:** Automatically detects and parses sharing links (e.g. Mediafire `/file/...` pages, Google Drive sharing links) into raw, direct high-speed file streams with accurate filename and file size detection.
* **Segmented Multi-Stream Acceleration:** Slices large files into 16 to 32 parallel HTTP `Range` chunks across independent TCP sockets, multiplying aggregate throughput across rate-limited queues.
* **Hidden Turbo Mode (Cloudflare WARP UDP Tunnel):**
  * Tucked neatly away inside Turbo Settings to keep the UI clean.
  * Tunnels multi-stream download packets through WireGuard UDP (port 443/2408), disguising downloads as high-priority real-time streaming traffic (identical to YouTube/Zoom) to defeat Deep Packet Inspection (DPI).
  * Includes a prominent visual installer modal with progress feedback.
* **Modern High-Contrast Desktop UI:** Built with a high-contrast dark palette, clear hierarchy, real-time speed meter, ETA, percentage badges, and a live Stream Inspector Table.
* **True Resume Capability:** Never loses downloaded bytes on dropped connections; byte offsets are automatically preserved and stitched atomically upon completion.

---

## Architecture Overview

* **`ushcodex_dm.py`**: The primary GUI application built in Python/Tkinter with high-contrast styling and non-blocking event loops.
* **`url_resolver.py`**: Smart URL analyzer and parser for Mediafire, Google Drive, and generic direct streams.
* **`downloader.py`**: High-concurrency worker threads, chunk slicing, strict boundary validation, and file reconstruction engine.
* **`history_manager.py`**: Persistent download records, status tracking, and automated SHA-256 hash auditing.
* **`logger.py`**: Persistent process logging system (`ushcodex_dm.log`) and live UI diagnostics.
* **`warp_manager.py`**: Detection, installation, and management of Cloudflare WARP client.
* **`abu_network_audit.py`**: Diagnostic suite for testing TCP throughput, UDP reachability, and network QoS queues.
* **`Launch_Ushcodex_DM.bat`**: 1-click Windows launcher.
* **`udm-rust/`**: High-performance Rust edition featuring multi-stream engine, system clipboard auto-interception, local HTTP daemon, Chrome/Edge browser extension, and cross-platform compilation matrix. See [udm-rust/README.md](udm-rust/README.md).

---

## Quick Start

### 1. Requirements
* Python 3.10+
* `requests`
* `beautifulsoup4`

```bash
pip install requests beautifulsoup4
```

### 2. Launch
Double-click `Launch_Ushcodex_DM.bat` or run:
```bash
python ushcodex_dm.py
```

### 3. CLI Mode
You can also run headless multi-stream downloads from the terminal:
```bash
python turbo_download.py "https://example.com/largefile.zip" -c 16
```

---

## License
MIT License. Developed by Ushcodex.
