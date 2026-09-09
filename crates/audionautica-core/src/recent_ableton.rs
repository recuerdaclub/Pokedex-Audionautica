//! Discover recent Ableton Live `.als` projects from app history and Live preferences.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::AppResult;

const RECENT_SETTING_KEY: &str = "recent_als_paths";
const MAX_STORED: usize = 12;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentAbletonSet {
    pub path: String,
    pub name: String,
}

pub fn list_recent_ableton_sets(conn: &Connection, limit: usize) -> AppResult<Vec<RecentAbletonSet>> {
    let limit = limit.max(1).min(20);
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for path in load_app_recent_paths(conn)? {
        push_recent(&mut out, &mut seen, path, limit);
        if out.len() >= limit {
            return Ok(out);
        }
    }

    for path in discover_from_ableton_preferences(limit * 4) {
        push_recent(&mut out, &mut seen, path, limit);
        if out.len() >= limit {
            return Ok(out);
        }
    }

    for project in db::list_projects_by_recent(conn, limit * 2)? {
        push_recent(&mut out, &mut seen, PathBuf::from(project.ableton_set_path), limit);
        if out.len() >= limit {
            break;
        }
    }

    Ok(out)
}

pub fn record_ableton_set_opened(conn: &Connection, als_path: &str) -> AppResult<()> {
    db::set_setting(conn, "last_als_path", als_path)?;
    let path = PathBuf::from(als_path);
    let mut paths = load_app_recent_paths(conn)?;
    paths.retain(|p| p != &path);
    paths.insert(0, path);
    paths.truncate(MAX_STORED);
    let encoded = serde_json::to_string(
        &paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    db::set_setting(conn, RECENT_SETTING_KEY, &encoded)
}

fn push_recent(
    out: &mut Vec<RecentAbletonSet>,
    seen: &mut HashSet<String>,
    path: PathBuf,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    if !path.is_file() {
        return;
    }
    if path.extension().and_then(|e| e.to_str()) != Some("als") {
        return;
    }
    let key = normalize_path_key(&path);
    if !seen.insert(key) {
        return;
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Proyecto")
        .trim()
        .to_string();
    out.push(RecentAbletonSet {
        path: path.to_string_lossy().to_string(),
        name,
    });
}

fn load_app_recent_paths(conn: &Connection) -> AppResult<Vec<PathBuf>> {
    let raw = db::get_setting(conn, RECENT_SETTING_KEY)?;
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let parsed: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(parsed.into_iter().map(PathBuf::from).collect())
}

fn normalize_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn discover_from_ableton_preferences(scan_limit: usize) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for prefs in ableton_preferences_files() {
        if let Ok(data) = std::fs::read(&prefs) {
            paths.extend(extract_als_paths_from_bytes(&data));
        }
        if paths.len() >= scan_limit {
            break;
        }
    }
    paths
}

fn ableton_preferences_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(root) = ableton_prefs_root() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("Live ")))
                .collect();
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            for version_dir in versions.into_iter().take(2) {
                let prefs = version_dir.join("Preferences").join("Preferences.cfg");
                if prefs.is_file() {
                    files.push(prefs);
                }
                #[cfg(target_os = "macos")]
                {
                    let mac_prefs = version_dir.join("Preferences.cfg");
                    if mac_prefs.is_file() {
                        files.push(mac_prefs);
                    }
                }
            }
        }
    }
    files
}

fn ableton_prefs_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("Ableton"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|p| {
            PathBuf::from(p)
                .join("Library")
                .join("Preferences")
                .join("Ableton")
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".Ableton"))
    }
}

fn extract_als_paths_from_bytes(data: &[u8]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.extend(extract_utf8_als_paths(data));
    out.extend(extract_utf16le_als_paths(data));
    out
}

fn extract_utf8_als_paths(data: &[u8]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in data.iter().enumerate() {
        if b == 0 {
            if i > start {
                if let Ok(chunk) = std::str::from_utf8(&data[start..i]) {
                    if let Some(path) = als_path_from_text(chunk) {
                        out.push(path);
                    }
                }
            }
            start = i + 1;
        }
    }
    if start < data.len() {
        if let Ok(chunk) = std::str::from_utf8(&data[start..]) {
            if let Some(path) = als_path_from_text(chunk) {
                out.push(path);
            }
        }
    }
    out
}

fn extract_utf16le_als_paths(data: &[u8]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let pat = [0x2e_u8, 0x00, 0x61, 0x00, 0x6c, 0x00, 0x73, 0x00];
    let mut i = 0;
    while i + pat.len() <= data.len() {
        if data[i..i + pat.len()] == pat {
            if let Some(path) = utf16_path_ending_at(data, i + pat.len()) {
                out.push(path);
            }
        }
        i += 2;
    }
    out
}

fn utf16_path_ending_at(data: &[u8], end: usize) -> Option<PathBuf> {
    if end % 2 != 0 || end < 4 {
        return None;
    }
    let mut units = Vec::new();
    let mut pos = end;
    while pos >= 2 {
        pos -= 2;
        let unit = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if unit == 0 {
            break;
        }
        if unit < 0x20 {
            return None;
        }
        units.insert(0, unit);
        if units.len() > 260 {
            return None;
        }
    }
    String::from_utf16(&units).ok().and_then(|text| als_path_from_text(&text))
}

fn als_path_from_text(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    if trimmed.len() < 5 || trimmed.len() > 420 {
        return None;
    }
    if !trimmed.ends_with(".als") && !trimmed.ends_with(".ALS") {
        return None;
    }
    if !(trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains(':')) {
        return None;
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_utf8_als_path_from_binary_blob() {
        let blob = b"noise\x00C:\\Music\\Jam Session.als\x00more";
        let paths = extract_als_paths_from_bytes(blob);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].to_string_lossy().ends_with("Jam Session.als"));
    }

    #[test]
    fn extracts_utf16le_als_path_from_binary_blob() {
        let text = "D:\\Sets\\Neon Groove.als";
        let mut blob = Vec::new();
        for unit in text.encode_utf16() {
            blob.extend_from_slice(&unit.to_le_bytes());
        }
        blob.extend_from_slice(&[0, 0]);
        let utf16_paths = extract_utf16le_als_paths(&blob);
        assert_eq!(utf16_paths.len(), 1, "utf16 paths: {:?}", utf16_paths);
        let paths = extract_als_paths_from_bytes(&blob);
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("Neon Groove.als")));
    }
}
