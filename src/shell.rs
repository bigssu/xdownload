//! Opening files, folders and links through the Windows shell API.
//!
//! Not through `explorer.exe <path>`: Explorer parses its command line itself, splits paths
//! at commas and, when it cannot make sense of the rest, silently opens Documents instead.
use std::path::Path;
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_NO_ASSOCIATION};
use windows_sys::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems, ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// NUL-terminated UTF-16, as the Windows API takes strings.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The shell functions need COM on the calling thread; a repeated call is harmless.
fn init_com() {
    // SAFETY: no reserved pointer; the apartment lives as long as the short-lived thread.
    unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
}

/// Opens a file, folder or web link with whatever Windows uses for it. Returns at once: the
/// shell can take long over a network drive that is not answering, and this is called from
/// the UI thread.
pub fn open(target: &str) {
    let target = target.to_owned();
    std::thread::spawn(move || open_now(&target));
}

/// Opens `file`, or `folder` when the file is gone (moved, deleted, or none recorded). A file
/// with no app for it still opens Windows' "How do you want to open this file?". Returns at
/// once, like `open`: even asking whether the file exists can hang on a network drive.
pub fn open_or(file: &str, folder: &str) {
    let (file, folder) = (file.to_owned(), folder.to_owned());
    std::thread::spawn(move || open_now(if !file.is_empty() && Path::new(&file).exists() { &file } else { &folder }));
}

/// Shows `file` selected in its folder (the folder alone when the file has gone). Returns
/// at once, like `open`.
pub fn reveal(file: &Path) {
    let file = file.to_owned();
    std::thread::spawn(move || reveal_now(&file));
}

/// `open` on the calling thread. False when it failed.
fn open_now(target: &str) -> bool {
    init_com();
    match execute("open", target, SEE_MASK_FLAG_NO_UI) {
        Ok(()) => true,
        // No app for this kind of file: Windows' "How do you want to open this file?".
        Err(ERROR_NO_ASSOCIATION) => execute("openas", target, 0).is_ok(),
        Err(_) => false,
    }
}

/// One `ShellExecuteExW` call; the Windows error code when it fails.
fn execute(verb: &str, target: &str, flags: u32) -> Result<(), u32> {
    let (verb, target) = (wide(verb), wide(target));
    // SAFETY: an all-zero SHELLEXECUTEINFOW is valid (null pointers, no flags); both strings
    // are NUL-terminated and outlive the call.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    // NOASYNC: this thread ends right after the call, so the shell must finish starting the
    // app first, or a Store or DDE app (Movies & TV, some players) can be dropped. NO_UI
    // (when given): a missing path fails quietly instead of popping up an error box.
    info.fMask = SEE_MASK_NOASYNC | flags;
    info.lpVerb = verb.as_ptr();
    info.lpFile = target.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut info) } != 0 {
        Ok(())
    } else {
        // SAFETY: reads this thread's last error, set by the failed call above.
        Err(unsafe { GetLastError() })
    }
}

/// `reveal` on the calling thread: the folder holding `file` with the file selected, the
/// folder alone when the file has gone. False when nothing could be opened.
fn reveal_now(file: &Path) -> bool {
    // An empty path would resolve to the desktop root and open some unrelated window.
    if file.as_os_str().is_empty() {
        return false;
    }
    let path = wide(&file.to_string_lossy());
    init_com();
    // SAFETY: the path is NUL-terminated and the item list is freed below.
    unsafe {
        let item = ILCreateFromPathW(path.as_ptr());
        if !item.is_null() {
            let result = SHOpenFolderAndSelectItems(item, 0, std::ptr::null(), 0);
            ILFree(item);
            return result >= 0;
        }
    }
    file.parent().is_some_and(|folder| !folder.as_os_str().is_empty() && open_now(&folder.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_nul_terminated_utf16() {
        assert_eq!(wide("a,b"), [97, 44, 98, 0]);
        assert_eq!(wide("한").len(), 2);
        assert_eq!(wide(""), [0]);
    }

    #[test]
    fn a_missing_target_reports_failure_instead_of_opening_something_else() {
        assert!(!open_now(r"C:\xdownload2-no-such-folder\nothing, here"));
        assert!(!reveal_now(Path::new(r"Q:\xdownload2-no-such-drive\a, b.mp4")));
        assert!(!reveal_now(Path::new("")), "an empty path (a 1.0 history entry) opens nothing");
    }

    /// Opens real Explorer windows, so it only runs on request:
    /// `cargo test shell -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn a_path_with_commas_opens_its_own_folder() {
        let dir = std::env::temp_dir().join(format!("xdownload2,shell {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a, b #c.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(reveal_now(&file));
        assert!(open_now(&dir.to_string_lossy()));
        println!("check that windows show {}", dir.display());
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}
