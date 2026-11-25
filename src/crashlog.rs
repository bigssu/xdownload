//! Append panic diagnostics before invoking the previously installed hook.

use std::path::PathBuf;

pub fn install() {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        install_at(PathBuf::from(appdata).join("Xdownload2/logs/crash.log"));
    }
}

/// Explicit path for tests and hosts that choose their own diagnostics directory.
pub fn install_at(path: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Diagnostics must never prevent the original hook from running.
        let _ = (|| -> std::io::Result<()> {
            use std::io::Write;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            let thread = std::thread::current();
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("Non-string panic payload");
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "unknown".into());
            let backtrace = std::backtrace::Backtrace::force_capture();
            writeln!(
                file,
                "time={} thread={} ({:?}) message={} location={}\nbacktrace={}\n",
                chrono::Local::now().to_rfc3339(),
                thread.name().unwrap_or("unnamed"),
                thread.id(),
                message,
                location,
                backtrace
            )
        })();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_hook_appends_details_and_calls_previous_hook() {
        // Isolate the process-global hook from every other test.
        if std::env::var_os("XDOWNLOAD2_CRASHLOG_CHILD").is_some() {
            let path = std::path::PathBuf::from(std::env::var_os("XDOWNLOAD2_CRASHLOG_CHILD").unwrap());
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = calls.clone();
            std::panic::set_hook(Box::new(move |_| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }));
            install_at(path.clone());
            for _ in 0..2 {
                assert!(std::panic::catch_unwind(|| panic!("panic marker")).is_err());
            }
            let log = std::fs::read_to_string(path).unwrap();
            assert_eq!(log.matches("panic marker").count(), 2);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
            for marker in ["thread=", "time=", "crashlog.rs:", "backtrace="] {
                assert!(log.contains(marker), "{marker}: {log}");
            }
            return;
        }
        let path = std::path::PathBuf::from("target/crashlog_test/nested/crash.log");
        let _ = std::fs::remove_file(&path);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "crashlog::tests::panic_hook_appends_details_and_calls_previous_hook",
            ])
            .env("XDOWNLOAD2_CRASHLOG_CHILD", &path)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
