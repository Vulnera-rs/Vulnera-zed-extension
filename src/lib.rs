//! Vulnera Zed Extension
//!
//! Downloads and manages the `vulnera-adapter` native binary, then launches it
//! as a Language Server over stdio.
//!
//! ## Binary lifecycle
//! 1. On `language_server_command`, resolve the current OS/arch to a target triple.
//! 2. Check if `server/vulnera-adapter[.exe]` exists and its version matches the
//!    latest release fetched from GitHub (cached for 24 h in `server/cached-version.txt`).
//! 3. If stale or missing, download from GitHub Releases and make executable.
//! 4. Return a `Command` that spawns the binary with no extra arguments
//!    (the binary reads/writes stdio by default).
//!
//! ## Version resolution (priority order)
//! 1. `VULNERA_ADAPTER_VERSION` env var — explicit pin for CI / development.
//! 2. `server/cached-version.txt` if its timestamp is within 24 h.
//! 3. Live query to the GitHub Releases API; result is written to the cache.
//! 4. Stale cache value (network outage tolerance).
//! 5. `MINIMUM_ADAPTER_VERSION` as absolute floor.
//!
//! ## Other environment variable overrides
//! - `VULNERA_ADAPTER_PATH`  — absolute path to a pre-built binary (skips download entirely).
//! - `VULNERA_API_URL`       — API base URL forwarded to the server as an env var.
//! - `VULNERA_API_KEY`       — API key forwarded to the server as an env var.
//! - `VULNERA_LOG`           — tracing log filter forwarded to the server (default: `info`).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use zed_extension_api::http_client::{HttpMethod, HttpRequest, RedirectPolicy};
use zed_extension_api::{self as zed, Architecture, DownloadedFileType, Os, Result};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Absolute minimum version used when the GitHub API is unreachable and no
/// version has ever been cached locally.
const MINIMUM_ADAPTER_VERSION: &str = "0.1.1";

/// How many seconds a cached version stays fresh before we re-query GitHub.
const VERSION_CACHE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// GitHub repository that publishes `adapter-v*` releases.
const GITHUB_REPO: &str = "vulnera-rs/adapter";

/// Language server ID declared in `extension.toml`.
const SERVER_ID: &str = "vulnera";

// ── Extension state ───────────────────────────────────────────────────────────

struct VulneraExtension {
    /// Cached path to the installed binary, set after the first successful install.
    cached_binary: Option<String>,
}

// ── Platform resolution ───────────────────────────────────────────────────────

/// Maps a (Os, Architecture) pair to the release asset metadata.
struct PlatformInfo {
    /// Rust target triple, e.g. `x86_64-unknown-linux-gnu`.
    target_triple: &'static str,
    /// Full filename of the release asset on the GitHub release page.
    asset_name: &'static str,
    /// Whether the platform requires a `.exe` suffix.
    is_windows: bool,
}

fn resolve_platform(os: Os, arch: Architecture) -> Result<PlatformInfo> {
    match (os, arch) {
        (Os::Linux, Architecture::X8664) => Ok(PlatformInfo {
            target_triple: "x86_64-unknown-linux-gnu",
            asset_name: "vulnera-adapter-x86_64-unknown-linux-gnu",
            is_windows: false,
        }),
        (Os::Linux, Architecture::Aarch64) => Ok(PlatformInfo {
            target_triple: "aarch64-unknown-linux-gnu",
            asset_name: "vulnera-adapter-aarch64-unknown-linux-gnu",
            is_windows: false,
        }),
        (Os::Mac, Architecture::X8664) => Ok(PlatformInfo {
            target_triple: "x86_64-apple-darwin",
            asset_name: "vulnera-adapter-x86_64-apple-darwin",
            is_windows: false,
        }),
        (Os::Mac, Architecture::Aarch64) => Ok(PlatformInfo {
            target_triple: "aarch64-apple-darwin",
            asset_name: "vulnera-adapter-aarch64-apple-darwin",
            is_windows: false,
        }),
        (Os::Windows, Architecture::X8664) => Ok(PlatformInfo {
            target_triple: "x86_64-pc-windows-msvc",
            asset_name: "vulnera-adapter-x86_64-pc-windows-msvc.exe",
            is_windows: true,
        }),
        _ => Err(format!(
            "Vulnera: unsupported platform ({:?} / {:?}). \
             Build vulnera-adapter from source and set VULNERA_ADAPTER_PATH.",
            os, arch
        )),
    }
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn binary_path(platform: &PlatformInfo) -> String {
    if platform.is_windows {
        "server/vulnera-adapter.exe".to_string()
    } else {
        "server/vulnera-adapter".to_string()
    }
}

fn installed_version_path() -> &'static str {
    "server/installed-version.txt"
}

fn cached_latest_version_path() -> &'static str {
    "server/cached-version.txt"
}

fn cached_version_timestamp_path() -> &'static str {
    "server/cached-version-timestamp.txt"
}

// ── Installed-version marker ──────────────────────────────────────────────────

fn read_installed_version() -> Option<String> {
    fs::read_to_string(installed_version_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_installed_version(version: &str) {
    if let Err(e) = fs::write(installed_version_path(), version) {
        eprintln!("[Vulnera] Failed to write installed-version marker: {}", e);
    }
}

// ── Latest-version cache (with TTL) ──────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_cached_latest_version() -> Option<(String, u64)> {
    let version = fs::read_to_string(cached_latest_version_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    let timestamp: u64 = fs::read_to_string(cached_version_timestamp_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    Some((version, timestamp))
}

fn write_cached_latest_version(version: &str) {
    if let Err(e) = fs::write(cached_latest_version_path(), version) {
        eprintln!("[Vulnera] Failed to write cached-version: {}", e);
    }
    if let Err(e) = fs::write(cached_version_timestamp_path(), now_secs().to_string()) {
        eprintln!("[Vulnera] Failed to write cached-version timestamp: {}", e);
    }
}

// ── GitHub version fetch ──────────────────────────────────────────────────────

/// Query the GitHub Releases API and return the version string (without the
/// `adapter-v` prefix) of the latest stable `adapter-v*` release, or `None`
/// if the request fails or no matching release is found.
fn fetch_latest_adapter_version_from_github() -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/releases", GITHUB_REPO);

    let request = HttpRequest {
        url,
        method: HttpMethod::Get,
        headers: vec![
            (
                "User-Agent".to_string(),
                "vulnera-zed-extension".to_string(),
            ),
            (
                "Accept".to_string(),
                "application/vnd.github+json".to_string(),
            ),
        ],
        body: None,
        redirect_policy: RedirectPolicy::FollowAll,
    };

    // `fetch` returns Err on transport failures and non-2xx HTTP errors.
    let response = match zed_extension_api::http_client::fetch(&request) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Vulnera] GitHub API request failed: {}", e);
            return None;
        }
    };

    let body = match String::from_utf8(response.body) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[Vulnera] Failed to decode GitHub API response: {}", e);
            return None;
        }
    };

    // Guard against HTML error pages (non-JSON responses).
    if !body.trim_start().starts_with('[') {
        eprintln!("[Vulnera] GitHub API returned unexpected body (not a JSON array)");
        return None;
    }

    // Minimal JSON extraction — find the first non-draft, non-prerelease
    // entry whose `tag_name` starts with `"adapter-v"`.
    //
    // We avoid pulling in `serde_json` to keep the WASM binary small;
    // the GitHub releases list format is stable enough for this approach.
    parse_latest_stable_version(&body)
}

/// Extract the first stable `adapter-v{VERSION}` tag from a raw JSON string
/// that looks like the GitHub `/releases` endpoint response.
///
/// Returns the version number without the `adapter-v` prefix on success.
fn parse_latest_stable_version(json: &str) -> Option<String> {
    // Each release object contains "tag_name":"adapter-vX.Y.Z".
    // We scan for that pattern while skipping entries marked as draft or prerelease.
    //
    // The JSON array is ordered newest-first, so the first matching entry is
    // the version we want.
    let mut remaining = json;

    while let Some(tag_start) = remaining.find("\"tag_name\":") {
        let after_key = &remaining[tag_start + "\"tag_name\":".len()..];

        // Find the opening quote of the tag value.
        let value_start = after_key.find('"')? + 1;
        let value_slice = &after_key[value_start..];
        let value_end = value_slice.find('"')?;
        let tag_name = &value_slice[..value_end];

        if tag_name.starts_with("adapter-v") {
            // Now look ahead within the same object for "prerelease":true or
            // "draft":true. Objects are separated by `},{` in the array.
            // We look at the substring up to the next top-level `},{`.
            let object_end = remaining[tag_start..]
                .find("},{")
                .unwrap_or(remaining.len() - tag_start);
            let object_slice = &remaining[tag_start..tag_start + object_end];

            let is_prerelease = object_slice.contains("\"prerelease\":true");
            let is_draft = object_slice.contains("\"draft\":true");

            if !is_prerelease && !is_draft {
                let version = tag_name.trim_start_matches("adapter-v").to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }

        // Advance past this tag_name occurrence.
        remaining = &remaining[tag_start + "\"tag_name\":".len()..];
    }

    None
}

// ── Version resolution ────────────────────────────────────────────────────────

/// Resolve the adapter version to use, applying the priority chain documented
/// at the top of this module.
fn resolve_adapter_version(shell_env: &[(String, String)]) -> String {
    // 1. Env var pin.
    if let Some((_, v)) = shell_env
        .iter()
        .find(|(k, _)| k == "VULNERA_ADAPTER_VERSION")
    {
        let v = v.trim();
        if !v.is_empty() {
            eprintln!("[Vulnera] Adapter version from env override: {}", v);
            return v.to_string();
        }
    }

    let now = now_secs();

    // 2. Fresh cache hit.
    if let Some((cached, fetched_at)) = read_cached_latest_version()
        && now.saturating_sub(fetched_at) < VERSION_CACHE_TTL_SECS
    {
        eprintln!(
            "[Vulnera] Adapter version from cache (age {}s): {}",
            now.saturating_sub(fetched_at),
            cached
        );
        return cached;
    }

    // 3. Live fetch.
    eprintln!("[Vulnera] Fetching latest adapter version from GitHub…");
    if let Some(fetched) = fetch_latest_adapter_version_from_github() {
        eprintln!("[Vulnera] Latest adapter version from GitHub: {}", fetched);
        write_cached_latest_version(&fetched);
        return fetched;
    }

    // 4. Stale cache fallback.
    if let Some((cached, _)) = read_cached_latest_version() {
        eprintln!(
            "[Vulnera] GitHub fetch failed; using stale cached version: {}",
            cached
        );
        return cached;
    }

    // 5. Absolute floor.
    eprintln!(
        "[Vulnera] GitHub fetch failed and no cache; falling back to minimum: {}",
        MINIMUM_ADAPTER_VERSION
    );
    MINIMUM_ADAPTER_VERSION.to_string()
}

// ── Download ──────────────────────────────────────────────────────────────────

fn download_url(platform: &PlatformInfo, version: &str) -> String {
    format!(
        "https://github.com/{}/releases/download/adapter-v{}/{}",
        GITHUB_REPO, version, platform.asset_name
    )
}

fn download_binary(platform: &PlatformInfo, version: &str) -> Result<()> {
    if let Err(e) = fs::create_dir_all("server") {
        return Err(format!(
            "Vulnera: failed to create server/ directory: {}",
            e
        ));
    }

    let url = download_url(platform, version);
    let dest = binary_path(platform);

    eprintln!(
        "[Vulnera] Downloading vulnera-adapter {} ({}) from {}",
        version, platform.target_triple, url
    );

    zed::download_file(&url, &dest, DownloadedFileType::Uncompressed)
        .map_err(|e| format!("Vulnera: download failed for {}: {}", url, e))?;

    if !platform.is_windows {
        zed::make_file_executable(&dest)
            .map_err(|e| format!("Vulnera: chmod +x failed for {}: {}", dest, e))?;
    }

    write_installed_version(version);

    eprintln!(
        "[Vulnera] vulnera-adapter {} installed at {}",
        version, dest
    );

    Ok(())
}

// ── Binary resolution ─────────────────────────────────────────────────────────

fn ensure_binary(platform: &PlatformInfo, version: &str) -> Result<String> {
    let dest = binary_path(platform);
    let installed = read_installed_version();
    let binary_exists = PathBuf::from(&dest).exists();

    let needs_download = !binary_exists || installed.as_deref() != Some(version);

    if needs_download {
        download_binary(platform, version)?;
    } else {
        eprintln!(
            "[Vulnera] vulnera-adapter {} already installed ({})",
            version, dest
        );
    }

    Ok(dest)
}

// ── Extension implementation ──────────────────────────────────────────────────

impl zed::Extension for VulneraExtension {
    fn new() -> Self {
        VulneraExtension {
            cached_binary: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!(
                "Vulnera: unknown language server id '{}'",
                language_server_id.as_ref()
            ));
        }

        let shell_env: Vec<(String, String)> = worktree.shell_env();

        // ── 1. Allow hard override for development / CI ───────────────────────
        if let Some((_, override_path)) =
            shell_env.iter().find(|(k, _)| k == "VULNERA_ADAPTER_PATH")
        {
            let p = override_path.trim();
            if !p.is_empty() {
                eprintln!("[Vulnera] Using VULNERA_ADAPTER_PATH override: {}", p);
                return Ok(build_command(p.to_string(), &shell_env));
            }
        }

        // ── 2. Resolve platform ───────────────────────────────────────────────
        let (os, arch) = zed::current_platform();
        let platform = resolve_platform(os, arch)?;

        // ── 3. Resolve target version (dynamic) ──────────────────────────────
        let version = resolve_adapter_version(&shell_env);

        // ── 4. Ensure binary is installed ─────────────────────────────────────
        let binary = match &self.cached_binary {
            Some(p) if PathBuf::from(p).exists() => {
                // Re-validate version in case the extension was updated in-place.
                if read_installed_version().as_deref() == Some(version.as_str()) {
                    p.clone()
                } else {
                    let new_path = ensure_binary(&platform, &version)?;
                    self.cached_binary = Some(new_path.clone());
                    new_path
                }
            }
            _ => {
                let new_path = ensure_binary(&platform, &version)?;
                self.cached_binary = Some(new_path.clone());
                new_path
            }
        };

        // ── 5. Build command with forwarded environment ───────────────────────
        Ok(build_command(binary, &shell_env))
    }
}

/// Build a `zed::Command` for the given binary path, forwarding relevant env
/// vars from the worktree shell environment.
fn build_command(binary: String, shell_env: &[(String, String)]) -> zed::Command {
    const FORWARDED_KEYS: &[&str] = &["VULNERA_API_URL", "VULNERA_API_KEY", "VULNERA_LOG"];

    let mut env: Vec<(String, String)> = shell_env
        .iter()
        .filter(|(k, v)| FORWARDED_KEYS.contains(&k.as_str()) && !v.trim().is_empty())
        .cloned()
        .collect();

    if !env.iter().any(|(k, _)| k == "VULNERA_LOG") {
        env.push(("VULNERA_LOG".to_string(), "info".to_string()));
    }

    zed::Command {
        command: binary,
        args: vec![],
        env,
    }
}

zed::register_extension!(VulneraExtension);

#[cfg(test)]
mod tests {
    use super::parse_latest_stable_version;

    #[test]
    fn parses_stable_release() {
        let json = r#"[
            {"tag_name":"adapter-v0.2.0","prerelease":false,"draft":false,"body":"notes"},
            {"tag_name":"adapter-v0.1.1","prerelease":false,"draft":false,"body":"notes"}
        ]"#;
        assert_eq!(parse_latest_stable_version(json), Some("0.2.0".to_string()));
    }

    #[test]
    fn skips_prerelease() {
        let json = r#"[
            {"tag_name":"adapter-v0.2.0-rc1","prerelease":true,"draft":false,"body":"notes"},
            {"tag_name":"adapter-v0.1.1","prerelease":false,"draft":false,"body":"notes"}
        ]"#;
        assert_eq!(parse_latest_stable_version(json), Some("0.1.1".to_string()));
    }

    #[test]
    fn skips_draft() {
        let json = r#"[
            {"tag_name":"adapter-v0.2.0","prerelease":false,"draft":true,"body":"notes"},
            {"tag_name":"adapter-v0.1.1","prerelease":false,"draft":false,"body":"notes"}
        ]"#;
        assert_eq!(parse_latest_stable_version(json), Some("0.1.1".to_string()));
    }

    #[test]
    fn ignores_non_adapter_tags() {
        let json = r#"[
            {"tag_name":"v1.0.0","prerelease":false,"draft":false,"body":"notes"},
            {"tag_name":"adapter-v0.1.1","prerelease":false,"draft":false,"body":"notes"}
        ]"#;
        assert_eq!(parse_latest_stable_version(json), Some("0.1.1".to_string()));
    }

    #[test]
    fn returns_none_on_empty_list() {
        assert_eq!(parse_latest_stable_version("[]"), None);
    }
}
