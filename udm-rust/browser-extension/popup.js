const toggle = document.getElementById("toggle-enabled");
const statusBadge = document.getElementById("daemon-status");

// Load current state
chrome.storage.local.get({ enabled: true }, (items) => {
    toggle.checked = items.enabled;
});

toggle.addEventListener("change", () => {
    chrome.storage.local.set({ enabled: toggle.checked });
});

// Check if UDM Rust daemon is running
fetch("http://127.0.0.1:19800/")
    .then(r => r.json())
    .then(data => {
        if (data.udm_status === "active") {
            statusBadge.textContent = "Online (Rust)";
            statusBadge.style.color = "#22c55e";
        } else {
            statusBadge.textContent = "Standby";
            statusBadge.style.color = "#f59e0b";
        }
    })
    .catch(() => {
        statusBadge.textContent = "Offline";
        statusBadge.style.color = "#ef4444";
    });
