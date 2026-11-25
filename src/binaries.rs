//! Downloads yt-dlp and ffmpeg on first use and caches them in %APPDATA%\Xdownload2\bin.
//!
//! Nothing is bundled in the executable, so the first run fetches both. yt-dlp keeps itself
//! current afterwards (`-U`), which is how the app follows YouTube changes.
use crate::i18n::fill;
use crate::proc::Job;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const YTDLP_EXE: &str = "yt-dlp.exe";
const FFMPEG_EXE: &str = "ffmpeg.exe";
const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_SUMS_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS";
/// BtbN win64 build; only `bin/ffmpeg.exe` is taken out of the archive.
const FFMPEG_ZIP: &str = "ffmpeg-master-latest-win64-gpl.zip";
const FFMPEG_BASE_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/";
const FFMPEG_SUMS: &str = "checksums.sha256";

type Res<T> = Result<T, String>;

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// HTTPS through Windows' own TLS stack and certificate store.
fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(60)))
        // A stalled transfer must fail (and let a cancel finish) instead of waiting forever.
        .timeout_per_read(Some(std::time::Duration::from_secs(60)))
        .build()
        .new_agent()
}

/// The SHA-256 listed for exactly `filename` in sha256sum output (`<hex>  <name>`, the name
/// optionally marked binary with `*`). Exact matching keeps `win64-gpl` apart from
/// `win64-gpl-shared`.
pub fn expected_sha256(sums: &str, filename: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let (hash, name) = (parts.next()?, parts.next()?);
        (parts.next().is_none() && name.trim_start_matches('*') == filename && hash.len() == 64).then(|| hash.to_lowercase())
    })
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Deletes `path` and fails when its SHA-256 differs from `expected`.
pub fn verify_sha256(path: &Path, expected: &str, label: &str) -> Res<()> {
    if sha256_file(path).map_err(err)? == expected {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    Err(fill("checksum_fail", &[("name", label)]))
}

/// Temporary download names carry the process id, so two app windows preparing the same
/// binary never write into each other's file. `clear_stale` knows them by these parts.
const PART_END: &str = ".part";
const ZIP_START: &str = "ffmpeg.";
const ZIP_END: &str = ".zip";

fn part_suffix() -> String {
    format!(".{}{PART_END}", std::process::id())
}

fn ffmpeg_zip(dir: &Path) -> PathBuf {
    dir.join(format!("{ZIP_START}{}{ZIP_END}", std::process::id()))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// A small text file over HTTPS (at most 10 MB, ureq's limit for `read_to_string`).
pub fn fetch_text(url: &str) -> Res<String> {
    agent().get(url).call().map_err(err)?.body_mut().read_to_string().map_err(err)
}

fn fetch_expected(sums_url: &str, filename: &str) -> Res<String> {
    let sums = fetch_text(sums_url)?;
    expected_sha256(&sums, filename).ok_or_else(|| fill("checksum_missing", &[("name", filename)]))
}

/// Copies until the end of `from`, reporting the fraction done; stops early when cancelled.
fn copy_with_progress(from: &mut dyn Read, to: &mut dyn Write, total: Option<u64>, job: &Job, on_progress: &mut dyn FnMut(f32)) -> Res<()> {
    let mut buffer = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        if job.cancelled() {
            return Err("cancelled".into());
        }
        let n = from.read(&mut buffer).map_err(err)?;
        if n == 0 {
            return Ok(());
        }
        to.write_all(&buffer[..n]).map_err(err)?;
        done += n as u64;
        if let Some(total) = total.filter(|total| *total > 0) {
            on_progress((done as f32 / total as f32).min(1.0));
        }
    }
}

/// Streams `url` into `dest.part`, verifies it, then renames it into place — a damaged or
/// tampered download never becomes `dest`.
fn download_file(agent: &ureq::Agent, url: &str, dest: &Path, expected: &str, label: &str, job: &Job, on_progress: &mut dyn FnMut(f32)) -> Res<()> {
    let part = with_suffix(dest, &part_suffix());
    let result = (|| {
        let mut response = agent.get(url).call().map_err(err)?;
        let total = response.body().content_length();
        let mut file = std::fs::File::create(&part).map_err(err)?;
        copy_with_progress(&mut response.body_mut().as_reader(), &mut file, total, job, on_progress)?;
        file.sync_all().map_err(err)?;
        drop(file);
        verify_sha256(&part, expected, label)?;
        std::fs::rename(&part, dest).map_err(err)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// Writes the archive's `bin/ffmpeg.exe` to `dest` (through `dest.part`, so an interrupted
/// extraction never leaves a broken ffmpeg.exe to be reused).
pub fn extract_ffmpeg(zip_path: &Path, dest: &Path) -> Res<()> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(err)?).map_err(err)?;
    let member = archive.file_names().find(|name| name.ends_with("/bin/ffmpeg.exe")).map(str::to_owned)
        .ok_or_else(|| fill("zip_no_ffmpeg", &[]))?;
    let part = with_suffix(dest, &part_suffix());
    let result = (|| {
        let mut source = archive.by_name(&member).map_err(err)?;
        let mut file = std::fs::File::create(&part).map_err(err)?;
        std::io::copy(&mut source, &mut file).map_err(err)?;
        file.sync_all().map_err(err)?;
        drop(file);
        std::fs::rename(&part, dest).map_err(err)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn bin_dir() -> Res<PathBuf> {
    let dir = crate::settings::app_dir().ok_or("APPDATA is not set")?.join("bin");
    std::fs::create_dir_all(&dir).map_err(err)?;
    // Leftovers only come from an earlier run, so one look per run is enough.
    static CLEARED: std::sync::Once = std::sync::Once::new();
    CLEARED.call_once(|| crate::settings::clear_stale(&dir, std::time::Duration::from_secs(600), is_temporary));
    Ok(dir)
}

/// A download or extraction in progress, or one a closed run left behind.
fn is_temporary(name: &str) -> bool {
    name.ends_with(PART_END) || (name.starts_with(ZIP_START) && name.ends_with(ZIP_END))
}

/// Deletes the cached yt-dlp and ffmpeg so they are downloaded again when next needed (the
/// way out of a damaged or broken copy). False when a file could not be removed.
pub fn forget() -> bool {
    // try_lock: this runs on the UI thread and must not wait for a download in progress. A
    // lock poisoned by an earlier panic is still usable, as in `ensure_*`.
    let _guard = match PREPARING.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return false,
    };
    bin_dir().is_ok_and(|dir| retire(&dir, &[YTDLP_EXE, FFMPEG_EXE]))
}

/// Moves `names` in `dir` out of the way, all or none, and deletes them when it can. A tool
/// another window is running cannot be deleted but can be renamed; the renamed copy ends in
/// `.part`, so a later run clears it once nothing uses it.
fn retire(dir: &Path, names: &[&str]) -> bool {
    let old_suffix = format!(".{}.old{PART_END}", std::process::id());
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in names {
        let (path, old) = (dir.join(name), dir.join(format!("{name}{old_suffix}")));
        match std::fs::rename(&path, &old) {
            Ok(()) => moved.push((path, old)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                // Put back what was already moved: never one tool gone and the other kept.
                for (path, old) in moved {
                    let _ = std::fs::rename(old, path);
                }
                return false;
            }
        }
    }
    for (_, old) in moved {
        let _ = std::fs::remove_file(old);
    }
    true
}

/// One preparation at a time inside this process: queue items that start together wait here
/// and then find the binary already cached.
static PREPARING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Path of yt-dlp.exe, downloading it first when missing.
pub fn ensure_ytdlp(job: &Job, on_progress: &mut dyn FnMut(f32)) -> Res<String> {
    let _guard = PREPARING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = bin_dir()?.join(YTDLP_EXE);
    if !path.exists() {
        let agent = agent();
        let expected = fetch_expected(YTDLP_SUMS_URL, YTDLP_EXE)?;
        download_file(&agent, YTDLP_URL, &path, &expected, "yt-dlp", job, on_progress)?;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Path of ffmpeg.exe, downloading and unpacking it first when missing.
pub fn ensure_ffmpeg(job: &Job, on_progress: &mut dyn FnMut(f32)) -> Res<String> {
    let _guard = PREPARING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = bin_dir()?;
    let path = dir.join(FFMPEG_EXE);
    if !path.exists() {
        let agent = agent();
        // Verifying the archive extends trust to the ffmpeg.exe inside it.
        let expected = fetch_expected(&format!("{FFMPEG_BASE_URL}{FFMPEG_SUMS}"), FFMPEG_ZIP)?;
        let zip_path = ffmpeg_zip(&dir);
        let result = download_file(&agent, &format!("{FFMPEG_BASE_URL}{FFMPEG_ZIP}"), &zip_path, &expected, "ffmpeg", job, on_progress)
            .and_then(|()| extract_ffmpeg(&zip_path, &path));
        let _ = std::fs::remove_file(&zip_path);
        result?;
    }
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xdownload2_bin_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn checksum_lookup_matches_the_exact_file_name() {
        let other = "A".repeat(64);
        let sums = format!("{other}  ffmpeg-master-latest-win64-gpl-shared.zip\n{ABC}  ffmpeg-master-latest-win64-gpl.zip\n{other} *yt-dlp.exe\nshort  x.zip\n");
        assert_eq!(expected_sha256(&sums, "ffmpeg-master-latest-win64-gpl.zip").as_deref(), Some(ABC));
        assert_eq!(expected_sha256(&sums, "yt-dlp.exe"), Some(other.to_lowercase()));
        assert_eq!(expected_sha256(&sums, "win64-gpl.zip"), None);
        assert_eq!(expected_sha256(&sums, "x.zip"), None, "a malformed hash is not a checksum");
        assert_eq!(expected_sha256("<html>Not Found</html>", "yt-dlp.exe"), None);
    }

    #[test]
    fn verification_keeps_good_files_and_deletes_bad_ones() {
        let dir = temp("verify");
        let file = dir.join("abc.bin");
        std::fs::write(&file, "abc").unwrap();
        assert_eq!(sha256_file(&file).unwrap(), ABC);
        assert!(verify_sha256(&file, ABC, "abc").is_ok() && file.exists());
        assert!(verify_sha256(&file, &"0".repeat(64), "abc").is_err());
        assert!(!file.exists(), "a file that fails verification must not survive");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copy_reports_progress_and_stops_when_cancelled() {
        let job = Job::default();
        let (mut out, mut seen) = (Vec::new(), Vec::new());
        copy_with_progress(&mut &[7u8; 100][..], &mut out, Some(100), &job, &mut |f| seen.push(f)).unwrap();
        assert_eq!((out.len(), seen.last().copied()), (100, Some(1.0)));
        // An unknown length downloads fine without progress.
        copy_with_progress(&mut &[7u8; 10][..], &mut Vec::new(), None, &job, &mut |_| panic!("no total, no progress")).unwrap();
        job.cancel();
        assert!(copy_with_progress(&mut &[7u8; 10][..], &mut Vec::new(), Some(10), &job, &mut |_| {}).is_err());
    }

    #[test]
    fn old_temporary_files_are_cleared_and_everything_else_is_kept() {
        let dir = temp("stale");
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for (name, old) in [("ffmpeg.111.zip", true), ("yt-dlp.exe.111.part", true), ("ffmpeg.222.zip", false), ("ffmpeg.3.zip.keep", true), ("ffmpeg.exe", true), ("yt-dlp.exe", true), ("notes.txt", true)] {
            let file = std::fs::File::create(dir.join(name)).unwrap();
            if old {
                file.set_modified(hour_ago).unwrap();
            }
        }
        crate::settings::clear_stale(&dir, std::time::Duration::from_secs(600), is_temporary);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["ffmpeg.222.zip", "ffmpeg.3.zip.keep", "ffmpeg.exe", "notes.txt", "yt-dlp.exe"], "only old temporary files go; a download in progress stays");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retiring_moves_every_tool_or_none() {
        let dir = temp("retire");
        for name in ["a.exe", "b.exe"] {
            std::fs::write(dir.join(name), name).unwrap();
        }
        assert!(retire(&dir, &["a.exe", "missing.exe", "b.exe"]), "a tool that is not there yet counts as gone");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        // A tool that cannot be moved (a folder of that name stands in for a locked file once
        // renaming onto an existing folder fails) leaves the other one in place too.
        std::fs::write(dir.join("a.exe"), "a").unwrap();
        std::fs::create_dir_all(dir.join("b.exe")).unwrap();
        std::fs::create_dir_all(dir.join(format!("b.exe.{}.old.part", std::process::id())).join("x")).unwrap();
        assert!(!retire(&dir, &["a.exe", "b.exe"]));
        assert_eq!(std::fs::read_to_string(dir.join("a.exe")).unwrap(), "a", "the first tool was put back");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ffmpeg_is_extracted_from_the_nested_bin_folder() {
        let dir = temp("zip");
        let zip_path = ffmpeg_zip(&dir);
        let write = |members: &[(&str, &str)]| {
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            for (name, body) in members {
                writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                writer.write_all(body.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        };
        write(&[("build/bin/ffprobe.exe", "probe"), ("build/bin/ffmpeg.exe", "the real one"), ("build/doc/ffmpeg.exe.txt", "doc")]);
        let dest = dir.join("ffmpeg.exe");
        extract_ffmpeg(&zip_path, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "the real one");
        assert!(!with_suffix(&dest, &part_suffix()).exists());
        write(&[("build/bin/ffprobe.exe", "probe")]);
        assert!(extract_ffmpeg(&zip_path, &dir.join("none.exe")).is_err());
        assert!(!dir.join("none.exe").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
