//! The system Trash. macOS tells us where it put each folder, so undo moves
//! it back; on Windows the Recycle Bin is searched by the folder's old path.

use crate::core::Trashed;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub fn to_trash(path: &Path) -> Result<Option<PathBuf>, String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let mut out = None;
    NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, Some(&mut out)).map_err(|e| e.localizedDescription().to_string())?;
    Ok(out.and_then(|u| u.path()).map(|p| PathBuf::from(p.to_string())))
}

#[cfg(target_os = "macos")]
pub fn restore(t: &Trashed) -> Result<(), String> {
    crate::core::restore_moved(t)
}

#[cfg(not(target_os = "macos"))]
pub fn to_trash(path: &Path) -> Result<Option<PathBuf>, String> {
    trash::delete(path).map(|_| None).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn restore(t: &Trashed) -> Result<(), String> {
    let want = Path::new(&t.path);
    let mut hits: Vec<_> = trash::os_limited::list().map_err(|e| e.to_string())?.into_iter().filter(|i| i.original_path() == want).collect();
    hits.sort_by_key(|i| i.time_deleted);
    let last = hits.pop().ok_or("emptied")?;
    trash::os_limited::restore_all([last]).map_err(|e| e.to_string())
}
