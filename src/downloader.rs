//! yt-dlp invocation and output parsing.
//!
//! Up to 1080p, H.264 (avc1) is preferred so the file plays everywhere; above that the higher
//! resolution wins (YouTube has no H.264 there). MP4 is guaranteed by remuxing, not re-encoding.
use crate::proc::{self, Job, CANCELLED};
use std::path::Path;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Mp4,
    Mp3,
    /// The original AAC audio, not re-encoded.
    M4a,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Mp4, Format::Mp3, Format::M4a];

    pub fn name(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Mp3 => "mp3",
            Format::M4a => "m4a",
        }
    }
}

/// Everything about one download that is fixed when it is queued.
#[derive(Clone, PartialEq, Debug)]
pub struct Options {
    pub format: Format,
    /// Largest video height; 0 = no limit.
    pub height: u32,
    pub subs: bool,
    pub embed_info: bool,
    pub playlist: bool,
    pub out_dir: String,
}

impl Options {
    /// "best", "1080p", ... as shown in the history.
    pub fn quality_name(&self) -> String {
        if self.format != Format::Mp4 || self.height == 0 { "best".into() } else { format!("{}p", self.height) }
    }
}

/// Marks the line that carries the finished file's path.
const FILE_MARK: &str = "XDL_FILE:";
/// Marks the line printed before each video downloads: `133+140|Title` (the streams, then the title).
const TITLE_MARK: &str = "XDL_TITLE:";
/// Marks a progress line: `133|  42.3%|1.00MiB/s|00:05` (the stream it is about, then the numbers).
const PROGRESS_MARK: &str = "XDL_PROG:";

fn mp4_format(height: u32) -> String {
    let h = if height == 0 { 9999 } else { height };
    // H.264 first, then any MP4, then anything (keeps live / DASH-only videos working).
    let compatible = format!(
        "bestvideo[height<={h}][vcodec^=avc1]+bestaudio[ext=m4a]/bestvideo[height<={h}][ext=mp4]+bestaudio/\
         best[height<={h}][ext=mp4]/best[height<={h}]/b[ext=mp4]/b"
    );
    if h <= 1080 {
        return compatible;
    }
    // Asked for more than 1080p: take it when it exists, whatever the codec.
    format!("bestvideo[height>1080][height<={h}]+bestaudio[ext=m4a]/bestvideo[height>1080][height<={h}]+bestaudio/{compatible}")
}

/// yt-dlp arguments (without the program itself).
pub fn build_args(url: &str, options: &Options, ffmpeg: &str) -> Vec<String> {
    // The video id keeps two videos with the same title from writing the same file.
    let template = Path::new(&options.out_dir).join("%(title)s [%(id)s].%(ext)s").to_string_lossy().into_owned();
    let title = format!("before_dl:{TITLE_MARK}%(format_id)s|%(title)s");
    let file = format!("after_move:{FILE_MARK}%(filepath)s");
    let progress = format!("download:{PROGRESS_MARK}%(info.format_id)s|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s");
    let mut args: Vec<String> = [
        "--newline", "--no-warnings", "--progress", "--progress-template", &progress,
        // --print would otherwise silence the progress lines.
        "--no-quiet", "--print", &title, "--print", &file,
        // Piped output otherwise uses the system code page, which garbles non-ASCII file names.
        "--encoding", "utf-8",
        "--concurrent-fragments", "4",
        "--ffmpeg-location", ffmpeg,
        "-o", &template,
        if options.playlist { "--yes-playlist" } else { "--no-playlist" },
    ].map(String::from).to_vec();
    match options.format {
        Format::Mp3 => args.extend(["-f", "bestaudio/best", "-x", "--audio-format", "mp3", "--audio-quality", "0"].map(String::from)),
        Format::M4a => args.extend(["-f", "bestaudio[ext=m4a]/bestaudio/best", "-x", "--audio-format", "m4a"].map(String::from)),
        Format::Mp4 => {
            args.extend(["-f".to_owned(), mp4_format(options.height)]);
            args.extend(["--merge-output-format", "mp4", "--remux-video", "mp4"].map(String::from));
            if options.subs {
                // Uploaded and automatic subtitles; no-keep-subs removes the .vtt files once embedded.
                args.extend(["--embed-subs", "--write-subs", "--write-auto-subs", "--sub-langs", "ko,en", "--compat-options", "no-keep-subs"].map(String::from));
            }
        }
    }
    if options.embed_info {
        args.extend(["--embed-thumbnail", "--embed-metadata"].map(String::from));
    }
    // "--" keeps a link that starts with a dash from being read as an option.
    args.extend(["--".to_owned(), url.to_owned()]);
    args
}

/// What one yt-dlp output line tells the UI.
#[derive(PartialEq, Debug)]
pub enum Event<'a> {
    /// Progress of one stream (`stream` is its format id; subtitles have none).
    Progress { stream: &'a str, percent: f32, speed: &'a str, eta: &'a str },
    /// The video about to download: its streams (`133+140` is video then audio) and title.
    Title { streams: &'a str, title: &'a str },
    /// Position inside a playlist.
    Item { index: u32, total: u32 },
    /// Full path of a finished file.
    File(&'a str),
    /// Why yt-dlp gave up ("Private video. Sign in if ...").
    Error(&'a str),
}

/// A value yt-dlp did not know ("NA", "Unknown B/s") reads as empty.
fn known(value: &str) -> &str {
    let value = value.trim();
    if value == "NA" || value.starts_with("Unknown") { "" } else { value }
}

pub fn parse(line: &str) -> Option<Event<'_>> {
    if let Some(path) = line.strip_prefix(FILE_MARK) {
        return Some(Event::File(path.trim()));
    }
    if let Some(rest) = line.strip_prefix(TITLE_MARK) {
        let (streams, title) = rest.split_once('|')?;
        // "NA" like the progress lines, so a site that names no streams still matches them.
        return Some(Event::Title { streams: known(streams), title: title.trim() });
    }
    if let Some(rest) = line.strip_prefix(PROGRESS_MARK) {
        let mut parts = rest.split('|');
        let (stream, percent, speed, eta) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        let percent = percent.trim().trim_end_matches('%').parse().ok()?;
        return Some(Event::Progress { stream: known(stream), percent, speed: known(speed), eta: known(eta) });
    }
    if let Some(reason) = line.strip_prefix("ERROR:") {
        // "[youtube] VIDEOID: Private video" -> "Private video": after an "[extractor]" tag the
        // first word is the video id. Without the tag ("Postprocessing: Conversion failed!")
        // the first word says where it failed and stays.
        let reason = reason.trim();
        let message = match reason.strip_prefix('[').and_then(|rest| rest.split_once("] ")) {
            Some((_, rest)) => match rest.split_once(": ") {
                Some((id, text)) if !id.contains(' ') => text,
                _ => rest,
            },
            None => reason,
        };
        return Some(Event::Error(message));
    }
    let item = line.strip_prefix("[download] Downloading item ")?;
    let (index, total) = item.split_once(" of ")?;
    Some(Event::Item { index: index.trim().parse().ok()?, total: total.trim().parse().ok()? })
}

/// Downloads one link. 0 on success, `CANCELLED` when cancelled, otherwise yt-dlp's exit code.
pub fn download(url: &str, options: &Options, ytdlp: &str, ffmpeg: &str, job: &Job, on_event: &mut dyn FnMut(Event)) -> std::io::Result<i32> {
    proc::run(ytdlp, &build_args(url, options, ffmpeg), job, |line| {
        if let Some(event) = parse(line) {
            on_event(event);
        }
    })
}

/// yt-dlp self-update; true when it ran to the end (already current counts). A failure just
/// leaves the old binary in place. Runs under the job, so a cancel stops a stuck update too.
pub fn update(ytdlp: &str, job: &Job) -> bool {
    matches!(proc::run(ytdlp, &["-U".to_owned()], job, |_| {}), Ok(0))
}

/// The self-update of this run: `None` until it was attempted, then whether it succeeded.
/// A failed attempt (offline, GitHub unreachable) is not repeated in this run: every
/// failing download would otherwise wait for its own network timeout. The next start tries again.
static UPDATED: Mutex<Option<bool>> = Mutex::new(None);

/// The self-update runs under this job, not under the item that asked for it: cancelling
/// that item must not kill an update others wait for. The app cancels it when it closes.
pub static UPDATE_JOB: Job = Job::new();

/// Updates yt-dlp at most once per run and says whether that succeeded. Downloads failing
/// at the same time queue up here: the first one updates, the others find it attempted.
pub fn update_once(ytdlp: &str, job: &Job) -> bool {
    let mut updated = UPDATED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *updated.get_or_insert_with(|| !job.cancelled() && update(ytdlp, &UPDATE_JOB))
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
pub fn download_with_auto_update(url: &str, options: &Options, ytdlp: &str, ffmpeg: &str, job: &Job, on_retry: &mut dyn FnMut(), on_event: &mut dyn FnMut(Event)) -> std::io::Result<i32> {
    retry_once(job, || download(url, options, ytdlp, ffmpeg, job, on_event), || {
        on_retry();
        let _ = update_once(ytdlp, job);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(format: Format, height: u32) -> Options {
        Options { format, height, subs: false, embed_info: false, playlist: false, out_dir: "C:/out".into() }
    }

    fn args(options: &Options) -> Vec<String> {
        build_args("https://youtu.be/x", options, "C:/bin/ffmpeg.exe")
    }

    fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
        &args[args.iter().position(|a| a == flag).unwrap() + 1]
    }

    fn has(args: &[String], flag: &str) -> bool {
        args.iter().any(|a| a == flag)
    }

    #[test]
    fn mp4_args_prefer_h264_within_the_height_limit_and_remux() {
        let a = args(&options(Format::Mp4, 720));
        let format = after(&a, "-f");
        assert!(format.starts_with("bestvideo[height<=720][vcodec^=avc1]+bestaudio[ext=m4a]/"));
        assert!(format.ends_with("/b[ext=mp4]/b"));
        assert_eq!(after(&a, "--merge-output-format"), "mp4");
        assert_eq!(after(&a, "--remux-video"), "mp4");
        assert_eq!(after(&a, "--concurrent-fragments"), "4");
        assert_eq!(after(&a, "--ffmpeg-location"), "C:/bin/ffmpeg.exe");
        let prints: Vec<&String> = a.iter().zip(&a[1..]).filter(|(flag, _)| *flag == "--print").map(|(_, value)| value).collect();
        assert_eq!(prints, ["before_dl:XDL_TITLE:%(format_id)s|%(title)s", "after_move:XDL_FILE:%(filepath)s"]);
        assert!(after(&a, "--progress-template").starts_with("download:XDL_PROG:%(info.format_id)s|"));
        assert!(after(&a, "-o").ends_with("%(title)s [%(id)s].%(ext)s") && after(&a, "-o").starts_with("C:/out"));
        assert!(has(&a, "--no-quiet") && has(&a, "--no-playlist") && !has(&a, "--yes-playlist"));
        assert_eq!(&a[a.len() - 2..], ["--", "https://youtu.be/x"]);
        assert!(after(&args(&options(Format::Mp4, 1080)), "-f").starts_with("bestvideo[height<=1080][vcodec^=avc1]"));
    }

    #[test]
    fn above_1080p_the_higher_resolution_comes_before_h264() {
        for (height, limit) in [(2160, 2160), (1440, 1440), (0, 9999)] {
            let a = args(&options(Format::Mp4, height));
            let format = after(&a, "-f");
            assert!(format.starts_with(&format!("bestvideo[height>1080][height<={limit}]+bestaudio[ext=m4a]/bestvideo[height>1080][height<={limit}]+bestaudio/")), "{format}");
            assert!(format.contains(&format!("/bestvideo[height<={limit}][vcodec^=avc1]+bestaudio[ext=m4a]/")), "falls back to the compatible chain");
        }
    }

    #[test]
    fn audio_formats_extract_and_ignore_video_options() {
        let mut o = options(Format::Mp3, 720);
        o.subs = true;
        let a = args(&o);
        assert_eq!((after(&a, "-f"), after(&a, "--audio-format"), after(&a, "--audio-quality")), ("bestaudio/best", "mp3", "0"));
        assert!(has(&a, "-x") && !has(&a, "--remux-video") && !has(&a, "--embed-subs"), "subtitles only go into MP4");
        let a = args(&options(Format::M4a, 0));
        assert_eq!((after(&a, "-f"), after(&a, "--audio-format")), ("bestaudio[ext=m4a]/bestaudio/best", "m4a"));
        assert_eq!(options(Format::M4a, 720).quality_name(), "best");
        assert_eq!(options(Format::Mp4, 720).quality_name(), "720p");
        assert_eq!(options(Format::Mp4, 0).quality_name(), "best");
    }

    #[test]
    fn optional_flags_follow_the_options() {
        let mut o = options(Format::Mp4, 0);
        (o.subs, o.embed_info, o.playlist) = (true, true, true);
        let a = args(&o);
        assert!(has(&a, "--embed-subs") && has(&a, "--write-auto-subs") && has(&a, "--write-subs"));
        assert_eq!(after(&a, "--compat-options"), "no-keep-subs");
        assert_eq!(after(&a, "--sub-langs"), "ko,en");
        assert!(has(&a, "--embed-thumbnail") && has(&a, "--embed-metadata") && has(&a, "--yes-playlist") && !has(&a, "--no-playlist"));
        let plain = args(&options(Format::Mp4, 0));
        assert!(!has(&plain, "--embed-subs") && !has(&plain, "--embed-thumbnail"));
    }

    #[test]
    fn output_lines_are_parsed() {
        // Lines as the cached yt-dlp printed them.
        assert_eq!(parse("XDL_PROG:133|  42.3%|  11.05MiB/s|00:05"), Some(Event::Progress { stream: "133", percent: 42.3, speed: "11.05MiB/s", eta: "00:05" }));
        assert_eq!(parse("XDL_PROG:140|100.0%|9.64MiB/s|NA"), Some(Event::Progress { stream: "140", percent: 100.0, speed: "9.64MiB/s", eta: "" }));
        assert_eq!(parse("XDL_PROG:NA| 50.0%|   657.59B/s|Unknown"), Some(Event::Progress { stream: "", percent: 50.0, speed: "657.59B/s", eta: "" }), "a subtitle has no stream");
        assert_eq!(parse("XDL_TITLE:133+140|노래 50% [live] | 제목"), Some(Event::Title { streams: "133+140", title: "노래 50% [live] | 제목" }));
        assert_eq!(parse("[download]  42.3% of 10.00MiB at 1.00MiB/s ETA 00:05"), None, "only the template line is read");
        assert_eq!(parse(r"[download] Destination: C:\out\노래 50% 제목.f137.mp4"), None, "file names no longer name the row");
        assert_eq!(parse("[download] Downloading item 3 of 12"), Some(Event::Item { index: 3, total: 12 }));
        assert_eq!(parse(r"XDL_FILE:C:\out\노래 제목.mp4"), Some(Event::File(r"C:\out\노래 제목.mp4")));
        assert_eq!(parse("[youtube] abc: Downloading webpage"), None);
        assert_eq!(parse("ERROR: [youtube] xxxxxxxxxxx: Video unavailable"), Some(Event::Error("Video unavailable")));
        assert_eq!(parse("ERROR: [youtube] abc: Private video. Sign in if you've been granted access"), Some(Event::Error("Private video. Sign in if you've been granted access")));
        assert_eq!(parse("ERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found"), Some(Event::Error("Unable to download webpage: HTTP Error 404: Not Found")));
        assert_eq!(parse("ERROR: Postprocessing: Conversion failed!"), Some(Event::Error("Postprocessing: Conversion failed!")));
        assert_eq!(parse("XDL_TITLE:NA|Clip"), Some(Event::Title { streams: "", title: "Clip" }));
        assert_eq!(parse("ERROR: unable to download video data: HTTP Error 403: Forbidden"), Some(Event::Error("unable to download video data: HTTP Error 403: Forbidden")));
        assert_eq!(parse("[download] Downloading playlist: mix"), None);
        assert_eq!(parse("[download] abc has already been downloaded"), None);
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
