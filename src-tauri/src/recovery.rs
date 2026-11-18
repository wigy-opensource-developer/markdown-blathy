//! Autosave snapshots and crash recovery (M9 task 4).
//!
//! While a document is dirty the frontend asks for a snapshot every ~30 s.
//! Each document has one slot — keyed by a stable FNV-1a hash of its exact
//! path, or the fixed `untitled` key — written atomically through the core's
//! `write_atomically` (a crash mid-write leaves the previous snapshot intact).
//!
//! On the next launch the frontend offers the newest snapshot whose content
//! differs from the file on disk and that is younger than [`MAX_AGE_SECS`].
//! A snapshot is deleted only on an explicit Discard, after a successful save,
//! or when it is stale/identical — never merely because the recovery check
//! lost a race, so an interrupted startup cannot lose edits.
//!
//! The exact path is stored as raw platform bytes (`path_bytes`), not the
//! lossy display string: restoring a document whose filename is not valid
//! UTF-8 keeps save-in-place pointed at the real file (SPEC §6).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use markdown_blathy_core::files::write_atomically;
use serde::{Deserialize, Serialize};

/// Snapshots older than this are deleted instead of offered.
const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;
/// Bump when the on-disk shape changes; other versions are discarded.
const SNAPSHOT_VERSION: u32 = 1;
/// The slot for a document that has never been saved.
const UNTITLED: &str = "untitled";

/// The on-disk shape (one JSON file per document slot).
#[derive(Debug, Serialize, Deserialize)]
struct StoredSnapshot {
    version: u32,
    markdown: String,
    /// Lossy display path for the UI; never used for file access.
    #[serde(default)]
    path: Option<String>,
    /// Exact platform path bytes (see `path_to_bytes`); absent when untitled.
    #[serde(default)]
    path_bytes: Option<Vec<u8>>,
    saved_at: u64,
}

/// What the frontend needs to offer a recovery.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    /// File stem inside the recovery directory; the token for restore/discard.
    pub id: String,
    pub markdown: String,
    pub path: Option<String>,
    pub saved_at: u64,
}

/// A restored snapshot: the text plus the exact path the backend must remember
/// for save-in-place. `exact` never crosses the IPC boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoredSnapshot {
    pub markdown: String,
    pub path: Option<String>,
    /// Backend-only: the frontend never sees (or needs) the exact path.
    #[serde(skip)]
    pub exact: Option<PathBuf>,
}

/// Write (or replace) the snapshot for `path` (None = untitled).
pub fn save(dir: &Path, path: Option<&Path>, markdown: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let stored = StoredSnapshot {
        version: SNAPSHOT_VERSION,
        markdown: markdown.to_owned(),
        path: path.map(|p| p.to_string_lossy().into_owned()),
        path_bytes: path.map(path_to_bytes),
        saved_at: now_secs(),
    };
    let json = serde_json::to_vec(&stored)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomically(&dir.join(format!("{}.json", key_for(path))), &json)
}

/// The newest snapshot worth offering, or None. Stale, corrupt and
/// already-on-disk snapshots are removed on the way.
pub fn take(dir: &Path) -> Option<RecoverySnapshot> {
    let mut best: Option<RecoverySnapshot> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let file = entry.path();
        if file.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = file.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        let Ok(raw) = fs::read(&file) else { continue };
        let Ok(stored) = serde_json::from_slice::<StoredSnapshot>(&raw) else {
            let _ = fs::remove_file(&file);
            continue;
        };
        if stored.version != SNAPSHOT_VERSION
            || now_secs().saturating_sub(stored.saved_at) > MAX_AGE_SECS
            || !differs_from_disk(&stored)
        {
            let _ = fs::remove_file(&file);
            continue;
        }
        let snapshot = RecoverySnapshot {
            id,
            markdown: stored.markdown,
            path: stored.path,
            saved_at: stored.saved_at,
        };
        if best
            .as_ref()
            .map(|current| snapshot.saved_at > current.saved_at)
            .unwrap_or(true)
        {
            best = Some(snapshot);
        }
    }
    best
}

/// Read one snapshot by id (without deleting it: it survives until an explicit
/// discard or a successful save, so a second crash still recovers).
pub fn restore(dir: &Path, id: &str) -> std::io::Result<Option<RestoredSnapshot>> {
    let Some(file) = snapshot_file(dir, id) else {
        return Ok(None);
    };
    let raw = match fs::read(&file) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let stored: StoredSnapshot = serde_json::from_slice(&raw)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let exact = stored.path_bytes.map(path_from_bytes);
    Ok(Some(RestoredSnapshot {
        markdown: stored.markdown,
        path: stored.path,
        exact,
    }))
}

/// Delete one snapshot (the user chose Discard).
pub fn discard(dir: &Path, id: &str) -> std::io::Result<()> {
    let Some(file) = snapshot_file(dir, id) else {
        return Ok(());
    };
    match fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Delete the snapshot for `path` and the untitled slot. Called after a save
/// (the edits are on disk) and by New (the old document is gone).
pub fn clear(dir: &Path, path: Option<&Path>) -> std::io::Result<()> {
    let mut result = Ok(());
    for key in [key_for(path), UNTITLED.to_owned()] {
        let file = dir.join(format!("{key}.json"));
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => result = Err(err),
        }
    }
    result
}

/// The slot name for a document: `untitled`, or a stable hash of the exact
/// path bytes. FNV-1a rather than `DefaultHasher`, whose output is not
/// guaranteed stable across Rust versions (a snapshot must survive updates).
fn key_for(path: Option<&Path>) -> String {
    match path {
        None => UNTITLED.to_owned(),
        Some(path) => format!("{:016x}", fnv1a(&path_to_bytes(path))),
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Validate an id from the frontend before touching the filesystem: no path
/// separators, no traversal, only the shape `key_for` produces (`untitled` or
/// 16 hex digits).
fn snapshot_file(dir: &Path, id: &str) -> Option<PathBuf> {
    let valid = id == UNTITLED || (id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit()));
    if !valid {
        return None;
    }
    Some(dir.join(format!("{id}.json")))
}

fn differs_from_disk(stored: &StoredSnapshot) -> bool {
    let Some(bytes) = &stored.path_bytes else {
        return true; // untitled: the text exists nowhere else
    };
    let path = path_from_bytes(bytes.clone());
    match fs::read_to_string(path) {
        Ok(on_disk) => on_disk != stored.markdown,
        Err(_) => true, // missing/unreadable: still worth offering
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The exact path as platform bytes (not the lossy display string).
#[cfg(unix)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(windows)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    PathBuf::from(std::ffi::OsString::from_wide(&units))
}

#[cfg(not(any(unix, windows)))]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(not(any(unix, windows)))]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "markdown-blathy-recovery-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_take_offers_the_snapshot_for_a_missing_file() {
        let dir = temp_dir("take");
        let doc = dir.join("notes.md");
        save(&dir, Some(&doc), "# Draft").unwrap();
        let snapshot = take(&dir).expect("snapshot offered");
        assert_eq!(snapshot.markdown, "# Draft");
        assert_eq!(snapshot.path.as_deref(), Some(doc.to_str().unwrap()));
        assert!(snapshot.saved_at > 0);
    }

    #[test]
    fn take_drops_a_snapshot_identical_to_the_file_on_disk() {
        let dir = temp_dir("identical");
        let doc = dir.join("notes.md");
        fs::write(&doc, "# Saved").unwrap();
        save(&dir, Some(&doc), "# Saved").unwrap();
        assert!(take(&dir).is_none());
        assert!(!has_snapshot_file(&dir), "the identical snapshot is pruned");
    }

    #[test]
    fn take_offers_a_snapshot_that_differs_from_disk() {
        let dir = temp_dir("differs");
        let doc = dir.join("notes.md");
        fs::write(&doc, "# Saved").unwrap();
        save(&dir, Some(&doc), "# Edited").unwrap();
        assert_eq!(take(&dir).unwrap().markdown, "# Edited");
    }

    #[test]
    fn untitled_snapshots_are_always_offered() {
        let dir = temp_dir("untitled");
        save(&dir, None, "draft").unwrap();
        let snapshot = take(&dir).expect("snapshot offered");
        assert_eq!(snapshot.id, UNTITLED);
        assert_eq!(snapshot.path, None);
    }

    #[test]
    fn newest_snapshot_wins() {
        let dir = temp_dir("newest");
        let first = dir.join("a.md");
        let second = dir.join("b.md");
        save(&dir, Some(&first), "older").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        save(&dir, Some(&second), "newer").unwrap();
        assert_eq!(take(&dir).unwrap().markdown, "newer");
    }

    #[test]
    fn restore_preserves_the_exact_path_and_keeps_the_file() {
        let dir = temp_dir("restore");
        let doc = dir.join("notes.md");
        save(&dir, Some(&doc), "# Draft").unwrap();
        let snapshot = take(&dir).unwrap();
        let restored = restore(&dir, &snapshot.id).unwrap().unwrap();
        assert_eq!(restored.markdown, "# Draft");
        assert_eq!(restored.exact.as_deref(), Some(doc.as_path()));
        assert_eq!(take(&dir).unwrap().id, snapshot.id, "restore keeps it");
    }

    #[test]
    fn discard_removes_the_snapshot() {
        let dir = temp_dir("discard");
        save(&dir, None, "draft").unwrap();
        discard(&dir, UNTITLED).unwrap();
        assert!(take(&dir).is_none());
        assert!(discard(&dir, UNTITLED).is_ok(), "discarding twice is fine");
    }

    #[test]
    fn clear_removes_the_document_and_untitled_slots() {
        let dir = temp_dir("clear");
        let doc = dir.join("notes.md");
        save(&dir, Some(&doc), "document").unwrap();
        save(&dir, None, "untitled").unwrap();
        clear(&dir, Some(&doc)).unwrap();
        assert!(take(&dir).is_none());
    }

    #[test]
    fn ids_from_the_frontend_cannot_escape_the_directory() {
        let dir = temp_dir("ids");
        assert!(snapshot_file(&dir, "../etc/passwd").is_none());
        assert!(snapshot_file(&dir, "not-a-key").is_none());
        assert!(snapshot_file(&dir, &"a".repeat(17)).is_none());
        assert!(snapshot_file(&dir, "0123456789abcdef").is_some());
        assert!(snapshot_file(&dir, UNTITLED).is_some());
    }

    #[test]
    fn stale_snapshots_are_pruned() {
        let dir = temp_dir("stale");
        let doc = dir.join("notes.md");
        save(&dir, Some(&doc), "old").unwrap();
        let file = dir.join(format!("{}.json", key_for(Some(&doc))));
        let mut stored: StoredSnapshot = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        stored.saved_at = 0; // ancient
        fs::write(&file, serde_json::to_vec(&stored).unwrap()).unwrap();
        assert!(take(&dir).is_none());
        assert!(!has_snapshot_file(&dir));
    }

    fn has_snapshot_file(dir: &Path) -> bool {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .any(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("json"))
    }
}
