use std::collections::HashMap;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, ACCEPT_RANGES};
use regex::Regex;
use url::Url;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub original_url: String,
    pub direct_url: String,
    pub filename: String,
    pub total_size: u64,
    pub supports_ranges: bool,
    pub content_type: String,
}

pub struct UrlResolver {
    client: reqwest::Client,
}

impl UrlResolver {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub fn sanitize_filename(filename: &str) -> String {
        let re = Regex::new(r#"[<>:"/\\|?*\x00-\x1F]"#).unwrap();
        let cleaned = re.replace_all(filename, "_").to_string();
        let trimmed = cleaned.trim_matches(|c| c == '.' || c == ' ');
        if trimmed.is_empty() {
            "download.bin".to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn extract_filename_from_headers(headers: &HeaderMap) -> Option<String> {
        if let Some(cd) = headers.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()) {
            // Check filename*=UTF-8''...
            let re_star = Regex::new(r#"(?i)filename\*=(?:UTF-8''|utf-8'')([^;]+)"#).unwrap();
            if let Some(caps) = re_star.captures(cd) {
                if let Some(m) = caps.get(1) {
                    let decoded = urlencoding::decode(m.as_str().trim_matches(|c| c == '"' || c == '\'')).unwrap_or_default();
                    if !decoded.is_empty() {
                        return Some(Self::sanitize_filename(&decoded));
                    }
                }
            }

            // Check filename="..."
            let re_std = Regex::new(r#"(?i)filename="?([^";]+)"?"#).unwrap();
            if let Some(caps) = re_std.captures(cd) {
                if let Some(m) = caps.get(1) {
                    let val = m.as_str().trim_matches(|c| c == '"' || c == '\'');
                    if !val.is_empty() {
                        return Some(Self::sanitize_filename(val));
                    }
                }
            }
        }
        None
    }

    pub fn extract_filename_from_url(raw_url: &str) -> Option<String> {
        if let Ok(parsed) = Url::parse(raw_url) {
            let path = parsed.path();
            let base = path.rsplit('/').next().unwrap_or("");
            if let Ok(unquoted) = urlencoding::decode(base) {
                if unquoted.contains('.') && !unquoted.ends_with(".php") && !unquoted.ends_with(".html") && !unquoted.ends_with(".htm") {
                    return Some(Self::sanitize_filename(&unquoted));
                }
            }
        }
        None
    }

    pub async fn resolve_mediafire(&self, url: &str) -> Option<(String, Option<String>)> {
        if let Ok(resp) = self.client.get(url).send().await {
            if let Ok(body) = resp.text().await {
                // Look for <a id="downloadButton" href="..."
                let re_btn = Regex::new(r#"id=["']downloadButton["'][^>]*href=["']([^"']+)["']"#).unwrap();
                if let Some(caps) = re_btn.captures(&body) {
                    let direct = caps.get(1)?.as_str().to_string();
                    let fn_opt = Self::extract_filename_from_url(&direct);
                    return Some((direct, fn_opt));
                }

                // Look for href inside download_link
                let re_aria = Regex::new(r#"href=["'](https?://download\d*\.mediafire\.com/[^"']+)["']"#).unwrap();
                if let Some(caps) = re_aria.captures(&body) {
                    let direct = caps.get(1)?.as_str().to_string();
                    let fn_opt = Self::extract_filename_from_url(&direct);
                    return Some((direct, fn_opt));
                }
            }
        }
        None
    }

    pub fn resolve_google_drive(url: &str) -> String {
        let re_file = Regex::new(r#"/file/d/([a-zA-Z0-9_-]+)"#).unwrap();
        if let Some(caps) = re_file.captures(url) {
            let id = &caps[1];
            return format!("https://drive.usercontent.google.com/download?id={}&export=download", id);
        }

        let re_id = Regex::new(r#"[?&]id=([a-zA-Z0-9_-]+)"#).unwrap();
        if let Some(caps) = re_id.captures(url) {
            let id = &caps[1];
            return format!("https://drive.usercontent.google.com/download?id={}&export=download", id);
        }

        url.to_string()
    }

    pub async fn resolve(&self, raw_url: &str, custom_headers: Option<HashMap<String, String>>) -> Result<ResolvedStream, String> {
        let trimmed = raw_url.trim();
        let mut target_url = trimmed.to_string();
        let mut suggested_filename: Option<String> = None;

        if target_url.contains("mediafire.com") && (target_url.contains("/file/") || target_url.contains("/download/")) {
            if let Some((direct, fn_hint)) = self.resolve_mediafire(&target_url).await {
                target_url = direct;
                suggested_filename = fn_hint;
            }
        } else if target_url.contains("drive.google.com") {
            target_url = Self::resolve_google_drive(&target_url);
        }

        let mut req_builder = self.client.head(&target_url);
        if let Some(headers) = custom_headers.clone() {
            for (k, v) in headers {
                if let (Ok(hk), Ok(hv)) = (k.parse::<reqwest::header::HeaderName>(), HeaderValue::from_str(&v)) {
                    req_builder = req_builder.header(hk, hv);
                }
            }
        }

        let resp_result = req_builder.send().await;
        let resp = match resp_result {
            Ok(r) => r,
            Err(_) => {
                // Try GET with Range bytes=0-0 if HEAD is disallowed
                let mut get_builder = self.client.get(&target_url).header("Range", "bytes=0-0");
                if let Some(headers) = custom_headers {
                    for (k, v) in headers {
                        if let (Ok(hk), Ok(hv)) = (k.parse::<reqwest::header::HeaderName>(), HeaderValue::from_str(&v)) {
                            get_builder = get_builder.header(hk, hv);
                        }
                    }
                }
                get_builder.send().await.map_err(|e| format!("Network connection failed: {}", e))?
            }
        };

        let final_url = resp.url().to_string();
        let headers = resp.headers().clone();

        let mut filename = suggested_filename;
        if filename.is_none() {
            filename = Self::extract_filename_from_headers(&headers);
        }
        if filename.is_none() {
            filename = Self::extract_filename_from_url(&final_url);
        }
        if filename.is_none() {
            filename = Self::extract_filename_from_url(trimmed);
        }

        let content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        if filename.is_none() {
            let ext = if content_type.contains("zip") {
                ".zip"
            } else if content_type.contains("mp4") {
                ".mp4"
            } else if content_type.contains("iso") {
                ".iso"
            } else if content_type.contains("pdf") {
                ".pdf"
            } else if content_type.contains("json") {
                ".json"
            } else {
                ".bin"
            };
            let ts = chrono::Utc::now().timestamp();
            filename = Some(format!("download_{}{}", ts, ext));
        }

        let total_size = headers
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let accepts_ranges = headers
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false)
            || total_size > 0;

        Ok(ResolvedStream {
            original_url: trimmed.to_string(),
            direct_url: final_url,
            filename: filename.unwrap_or_else(|| "download.bin".to_string()),
            total_size,
            supports_ranges: accepts_ranges,
            content_type,
        })
    }
}


