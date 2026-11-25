//! yt-dlp invocation and progress parsing.
//!
//! H.264 (avc1) is preferred so the file plays everywhere, and MP4 is guaranteed by remuxing
//! instead of re-encoding. MP3 is extracted from the best audio stream by ffmpeg.
use crate::proc::{self, Job, CANCELLED};
use std::path::Path;

/// Largest video height allowed for a quality name.
fn max_height(quality: &str) -> u32 {
    match quality {
        "1080p" => 1080,
        "720p" => 720,
        _ => 9999,
    }
}

/// H.264 first, then any MP4, then anything (keeps live / DASH-only / above-1080p videos working).
fn mp4_format(height: u32) -> String {
    format!(
        "bestvideo[height<={h}][vcodec^=avc1]+bestaudio[ext=m4a]/bestvideo[height<={h}][ext=mp4]+bestaudio/\
         best[height<={h}][ext=mp4]/best[height<={h}]/b[ext=mp4]/b",
        h = height
    )
}

pub struct Request<'a> {
    pub url: &'a str,
    pub mp3: bool,
    pub quality: &'a str,
    pub out_dir: &'a str,
    pub ytdlp: &'a str,
    pub ffmpeg: &'a str,
}

/// yt-dlp arguments (without the program itself).
pub fn build_args(request: &Request) -> Vec<String> {
    let template = Path::new(request.out_dir).join("%(title)s.%(ext)s").to_string_lossy().into_owned();
    let mut args: Vec<String> = [
        "--newline", "--no-warnings", "--progress",
        // Piped output otherwise uses the system code page, which garbles non-ASCII file names.
        "--encoding", "utf-8",
        "--concurrent-fragments", "4",
        "--ffmpeg-location", request.ffmpeg,
        "-o", &template,
    ].map(String::from).to_vec();
    if request.mp3 {
        args.extend(["-f", "bestaudio/best", "-x", "--audio-format", "mp3", "--audio-quality", "0"].map(String::from));
    } else {
        args.extend(["-f".to_owned(), mp4_format(max_height(request.quality))]);
        args.extend(["--merge-output-format", "mp4", "--remux-video", "mp4"].map(String::from));
    }
    // "--" keeps a link that starts with a dash from being read as an option.
    args.extend(["--".to_owned(), request.url.to_owned()]);
    args
}

/// `[download]  42.3% of 10.00MiB ...` → 42.3
pub fn percent(line: &str) -> Option<f32> {
    let rest = line.split_once("[download]")?.1.trim_start();
    rest.split_once('%')?.0.parse().ok()
}

/// `[download] Destination: C:\dir\name.mp4` → `name.mp4`
pub fn destination(line: &str) -> Option<&str> {
    let path = line.split_once("[download] Destination: ")?.1.trim();
    path.rsplit(['\\', '/']).next().filter(|name| !name.is_empty())
}

/// Downloads one link. 0 on success, `CANCELLED` when cancelled, otherwise yt-dlp's exit code.
pub fn download(request: &Request, job: &Job, on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
    proc::run(request.ytdlp, &build_args(request), job, on_line)
}

/// yt-dlp self-update. Returns at once when already current; a failure just leaves the old
/// binary in place. Runs under the job, so a cancel stops a stuck update too.
pub fn update(ytdlp: &str, job: &Job) {
    let _ = proc::run(ytdlp, &["-U".to_owned()], job, |_| {});
}

/// Runs `attempt`; on a failure (not success, not cancelled) runs `update` and tries once more.
fn retry_once(job: &Job, mut attempt: impl FnMut() -> std::io::Result<i32>, update: impl FnOnce()) -> std::io::Result<i32> {
    let code = attempt()?;
    if code == 0 || code == CANCELLED || job.cancelled() {
        return Ok(code);
    }
    update();
    if job.cancelled() {
        return Ok(CANCELLED);
    }
    attempt()
}

/// Downloads, and on a failure (bot detection, extractor errors) updates yt-dlp and retries
/// once: an outdated yt-dlp is the usual reason YouTube starts refusing.
pub fn download_with_auto_update(request: &Request, job: &Job, on_retry: &mut dyn FnMut(), on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
    retry_once(job, || download(request, job, on_line), || {
        on_retry();
        update(request.ytdlp, job);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(mp3: bool, quality: &'static str) -> Request<'static> {
        Request { url: "https://youtu.be/x", mp3, quality, out_dir: "C:/out", ytdlp: "yt-dlp.exe", ffmpeg: "C:/bin/ffmpeg.exe" }
    }

    fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
        &args[args.iter().position(|a| a == flag).unwrap() + 1]
    }

    #[test]
    fn mp4_args_prefer_h264_within_the_height_limit_and_remux() {
        let args = build_args(&request(false, "720p"));
        let format = after(&args, "-f");
        assert!(format.starts_with("bestvideo[height<=720][vcodec^=avc1]+bestaudio[ext=m4a]/"));
        assert!(format.ends_with("/b[ext=mp4]/b"));
        assert_eq!(after(&args, "--merge-output-format"), "mp4");
        assert_eq!(after(&args, "--remux-video"), "mp4");
        assert_eq!(after(&args, "--concurrent-fragments"), "4");
        assert_eq!(after(&args, "--ffmpeg-location"), "C:/bin/ffmpeg.exe");
        assert!(after(&args, "-o").ends_with("%(title)s.%(ext)s") && after(&args, "-o").starts_with("C:/out"));
        assert_eq!(&args[args.len() - 2..], ["--", "https://youtu.be/x"]);
        assert!(after(&build_args(&request(false, "1080p")), "-f").contains("height<=1080"));
        assert!(after(&build_args(&request(false, "best")), "-f").contains("height<=9999"));
        assert!(after(&build_args(&request(false, "bogus")), "-f").contains("height<=9999"));
    }

    #[test]
    fn mp3_args_extract_best_audio() {
        let args = build_args(&request(true, "720p"));
        assert_eq!(after(&args, "-f"), "bestaudio/best");
        assert_eq!(after(&args, "--audio-format"), "mp3");
        assert_eq!(after(&args, "--audio-quality"), "0");
        assert!(args.contains(&"-x".to_owned()) && !args.contains(&"--remux-video".to_owned()));
    }

    #[test]
    fn progress_and_destination_lines_are_parsed() {
        assert_eq!(percent("[download]  42.3% of 10.00MiB at 1.00MiB/s ETA 00:05"), Some(42.3));
        assert_eq!(percent("[download] 100% of 10.00MiB"), Some(100.0));
        assert_eq!(percent("[download] Destination: a 50% b.mp4"), None);
        assert_eq!(percent("[youtube] abc: Downloading webpage"), None);
        assert_eq!(destination(r"[download] Destination: C:\out\노래 제목.f137.mp4"), Some("노래 제목.f137.mp4"));
        assert_eq!(destination("[download] Destination: clip.webm"), Some("clip.webm"));
        assert_eq!(destination("[download]  42.3% of 10.00MiB"), None);
    }

    #[test]
    fn only_a_failure_updates_and_retries_once() {
        use std::cell::Cell;
        // (first exit code, cancel during update) -> (attempts, updates, result)
        for (first, cancel_in_update, expected) in [(0, false, (1, 0, 0)), (CANCELLED, false, (1, 0, CANCELLED)), (1, false, (2, 1, 1)), (1, true, (1, 1, CANCELLED))] {
            let job = Job::default();
            let (attempts, updates) = (Cell::new(0), Cell::new(0));
            let code = retry_once(&job, || {
                attempts.set(attempts.get() + 1);
                Ok(first)
            }, || {
                updates.set(updates.get() + 1);
                if cancel_in_update {
                    job.cancel();
                }
            }).unwrap();
            assert_eq!((attempts.get(), updates.get(), code), expected, "first exit code {first}");
        }
        // The retry's own result is what the caller sees.
        let mut codes = [1, 0].into_iter();
        assert_eq!(retry_once(&Job::default(), || Ok(codes.next().unwrap()), || {}).unwrap(), 0);
    }
}
