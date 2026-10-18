const UDM_ENDPOINT = "http://127.0.0.1:19800/api/download";

// Listen for browser download creation
chrome.downloads.onCreated.addListener(async (downloadItem) => {
    // Check if auto-interception is enabled
    const settings = await chrome.storage.local.get({ enabled: true });
    if (!settings.enabled) return;

    const url = downloadItem.finalUrl || downloadItem.url;
    if (!url || url.startsWith("blob:") || url.startsWith("data:")) return;

    console.log("[UDM Interceptor] Captured browser download:", url);

    // Cancel browser internal download immediately
    try {
        await chrome.downloads.cancel(downloadItem.id);
        await chrome.downloads.erase({ id: downloadItem.id });
    } catch (e) {
        console.warn("[UDM Interceptor] Notice on canceling browser download:", e);
    }

    // Forward payload to UDM Rust daemon
    const payload = {
        url: url,
        filename: downloadItem.filename ? downloadItem.filename.split(/[/\\]/).pop() : null,
        referer: downloadItem.referrer || null,
        user_agent: navigator.userAgent,
        auto_start: true,
        source: "Browser Extension (Chrome/Edge/Brave/Firefox)"
    };

    try {
        const resp = await fetch(UDM_ENDPOINT, {
            method: "POST",
            headers: {
                "Content-Type": "application/json"
            },
            body: JSON.stringify(payload)
        });

        if (resp.ok) {
            console.log("[UDM Interceptor] Successfully handed off download to UDM Rust!");
        } else {
            console.error("[UDM Interceptor] UDM Daemon returned status:", resp.status);
        }
    } catch (err) {
        console.error("[UDM Interceptor] Could not contact UDM Rust on port 19800. Ensure 'udm-rust daemon' is running!", err);
    }
});
