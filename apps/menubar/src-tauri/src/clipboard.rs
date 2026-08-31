//! the general pasteboard, reached directly rather than through a plugin

use tauri_nspanel::objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use tauri_nspanel::objc2_foundation::NSString;

/// not navigator.clipboard, a release build serves over tauri:// and whether
/// that counts as a secure context is not worth betting a copy button on.
/// NSPasteboard is not main-thread-only, so this can answer with the write's
/// own result instead of a dispatch that has not run yet
#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let pasteboard = NSPasteboard::generalPasteboard();

    // clearContents first, else the write lands behind whatever is there
    let wrote = unsafe {
        pasteboard.clearContents();
        pasteboard.setString_forType(&NSString::from_str(&text), NSPasteboardTypeString)
    };

    wrote
        .then_some(())
        .ok_or_else(|| "the pasteboard refused the write".to_owned())
}
