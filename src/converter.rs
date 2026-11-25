//! Local media conversion: ffmpeg turns videos and images into widely playable formats.
//! The original is kept and the new file is written next to it.
use crate::proc::{self, Job, CANCELLED};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const VIDEO_EXTS: [&str; 12] = ["3gp", "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mts", "ts", "webm", "wmv"];
pub const IMAGE_EXTS: [&str; 12] = ["avif", "bmp", "gif", "heic", "heif", "jfif", "jpeg", "jpg", "png", "tif", "tiff", "webp"];
/// Marks an MP4 this module made from another MP4.
const H264_MARK: &str = "_h264_";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VideoTarget {
    Mp4,
    Mp3,
    Gif,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ImageTarget {
    Jpg,
    Png,
    Webp,
}

/// What a source file is converted into; fixed when the item is queued.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    /// `height` 0 keeps the source size; `quality` 0 quality first, 1 balanced, 2 small file.
    Video { target: VideoTarget, height: u32, quality: u8 },
    Image(ImageTarget),
}

impl Target {
    pub fn is_video(self) -> bool {
        matches!(self, Target::Video { .. })
    }

    /// Extensions accepted as input.
    pub fn sources(self) -> &'static [&'static str] {
        if self.is_video() { &VIDEO_EXTS } else { &IMAGE_EXTS }
    }

    /// Extension of the output file.
    pub fn extension(self) -> &'static str {
        match self {
            Target::Video { target: VideoTarget::Mp4, .. } => "mp4",
            Target::Video { target: VideoTarget::Mp3, .. } => "mp3",
            Target::Video { target: VideoTarget::Gif, .. } => "gif",
            Target::Image(ImageTarget::Jpg) => "jpg",
            Target::Image(ImageTarget::Png) => "png",
            Target::Image(ImageTarget::Webp) => "webp",
        }
    }

    /// Suffix added when an MP4 is converted to MP4: `_h264_1080p`, `_h264_720p`, `_h264_src`.
    fn mp4_suffix(self) -> String {
        match self {
            Target::Video { height: 0, .. } => format!("{H264_MARK}src"),
            Target::Video { height, .. } => format!("{H264_MARK}{height}p"),
            Target::Image(_) => String::new(),
        }
    }

    fn accepts(self, path: &Path) -> bool {
        let Some(ext) = extension(path) else { return false };
        // An image already in the target format has nothing to convert.
        self.sources().contains(&ext.as_str()) && (self.is_video() || ext != self.extension())
    }
}

/// Which target a dropped file would be converted with: `Some(true)` video, `Some(false)` image.
pub fn is_video_file(path: &Path) -> Option<bool> {
    let ext = extension(path)?;
    if VIDEO_EXTS.contains(&ext.as_str()) {
        Some(true)
    } else {
        IMAGE_EXTS.contains(&ext.as_str()).then_some(false)
    }
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_lowercase())
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// An MP4 this module wrote earlier: `name_h264_1080p`, `name_h264_src-2`, ...
fn is_generated_video(path: &Path) -> bool {
    let stem = stem(path).to_lowercase();
    let Some((_, tail)) = stem.rsplit_once(H264_MARK) else { return false };
    let tail = match tail.rsplit_once('-') {
        Some((label, number)) if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => label,
        _ => tail,
    };
    tail == "src" || tail.strip_suffix('p').is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Files to convert from a list of files and folders.
///
/// A folder contributes the files directly inside it (and below, when `recursive`). Outputs
/// land in the same folder, so earlier MP4 outputs are skipped rather than converted again;
/// a file picked by hand is always taken.
pub fn collect_media_files(sources: &[PathBuf], target: Target, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for source in sources {
        if source.is_file() {
            if target.accepts(source) {
                files.push(source.clone());
            }
        } else {
            collect_folder(source, target, recursive, &mut files);
        }
    }
    files
}

fn collect_folder(folder: &Path, target: Target, recursive: bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    children.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()));
    let (folders, here): (Vec<PathBuf>, Vec<PathBuf>) = children.into_iter().partition(|path| path.is_dir());
    files.extend(here.into_iter().filter(|path| path.is_file() && target.accepts(path) && !(target.is_video() && is_generated_video(path))));
    if recursive {
        for folder in folders {
            collect_folder(&folder, target, recursive, files);
        }
    }
}

/// The names a conversion may write to, in order of preference: next to the source as
/// `name.ext` (`name_h264_720p.mp4` when the source already is an MP4), then `-2`, `-3`, ...
fn output_candidates(source: &Path, target: Target) -> impl Iterator<Item = PathBuf> {
    let ext = target.extension();
    let suffix = if extension(source).as_deref() == Some(ext) { target.mp4_suffix() } else { String::new() };
    let base = format!("{}{suffix}", stem(source));
    let folder = source.to_path_buf();
    std::iter::once(String::new()).chain((2..10000).map(|n| format!("-{n}"))).map(move |n| folder.with_file_name(format!("{base}{n}.{ext}")))
}

/// Takes the first free output name by creating the (empty) file. Creation fails when the
/// file exists, so conversions running side by side — in this window or another — never
/// share a name, and an existing file is never overwritten.
fn reserve_output(source: &Path, target: Target) -> std::io::Result<PathBuf> {
    for candidate in output_candidates(source, target) {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other(format!("no free output name for {}", source.display())))
}

/// H.264 encoder used for MP4 output.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Encoder {
    Cpu,
    Nvenc,
    Qsv,
    Amf,
}

impl Encoder {
    /// Short name shown next to a conversion.
    pub fn label(self) -> &'static str {
        match self {
            Encoder::Cpu => "CPU",
            Encoder::Nvenc => "GPU · NVIDIA",
            Encoder::Qsv => "GPU · Intel",
            Encoder::Amf => "GPU · AMD",
        }
    }

    fn codec(self) -> &'static str {
        match self {
            Encoder::Cpu => "libx264",
            Encoder::Nvenc => "h264_nvenc",
            Encoder::Qsv => "h264_qsv",
            Encoder::Amf => "h264_amf",
        }
    }

    /// Codec and rate-control arguments for a libx264 CRF of `crf`. Hardware encoders need a
    /// higher number for a similar file size (measured: NVENC at +3 wrote 1.8x the bytes).
    fn video_args(self, crf: u32) -> Vec<String> {
        let (crf, hw) = (crf.to_string(), (crf + if self == Encoder::Nvenc { 7 } else { 3 }).to_string());
        let args: Vec<&str> = match self {
            Encoder::Cpu => vec!["-c:v", "libx264", "-preset", "medium", "-crf", &crf, "-pix_fmt", "yuv420p"],
            Encoder::Nvenc => vec!["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", &hw, "-b:v", "0", "-pix_fmt", "yuv420p"],
            Encoder::Qsv => vec!["-c:v", "h264_qsv", "-global_quality", &hw, "-pix_fmt", "nv12"],
            Encoder::Amf => vec!["-c:v", "h264_amf", "-rc", "cqp", "-qp_i", &hw, "-qp_p", &hw, "-qp_b", &hw, "-pix_fmt", "yuv420p"],
        };
        args.into_iter().map(String::from).collect()
    }
}

/// The first hardware encoder that really encodes a frame on this PC (a listed encoder can
/// still fail without the matching card or driver), probed once per run.
pub fn detect_encoder(ffmpeg: &str) -> Encoder {
    static FOUND: OnceLock<Encoder> = OnceLock::new();
    *FOUND.get_or_init(|| {
        [Encoder::Nvenc, Encoder::Qsv, Encoder::Amf].into_iter().find(|encoder| {
            proc::command(ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=black:s=256x144:d=0.1", "-frames:v", "1", "-c:v", encoder.codec(), "-f", "null", "-"])
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .status().is_ok_and(|status| status.success())
        }).unwrap_or(Encoder::Cpu)
    })
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// ffmpeg arguments (without the program) for `target`.
pub fn build_args(source: &Path, dest: &Path, target: Target, encoder: Encoder) -> Vec<String> {
    let mut args = strings(&["-y", "-hide_banner", "-progress", "pipe:1", "-nostats", "-i"]);
    args.push(text(source));
    match target {
        Target::Video { target: VideoTarget::Mp4, height, quality } => {
            // H.264 needs even dimensions; a height limit never enlarges.
            let scale = if height == 0 { "scale=trunc(iw/2)*2:trunc(ih/2)*2".to_owned() } else { format!("scale=-2:min({height}\\,ih)") };
            args.extend(strings(&["-map", "0:v:0", "-map", "0:a:0?", "-vf", &scale]));
            args.extend(encoder.video_args([20, 23, 28][quality.min(2) as usize]));
            args.extend(strings(&["-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart"]));
        }
        Target::Video { target: VideoTarget::Mp3, .. } => args.extend(strings(&["-vn", "-c:a", "libmp3lame", "-q:a", "2"])),
        Target::Video { target: VideoTarget::Gif, .. } => args.extend(strings(&[
            "-an", "-vf", "fps=12,scale=480:-2:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse", "-loop", "0",
        ])),
        // -update 1: the output name is taken literally (a `%03d` in it is not a pattern).
        Target::Image(ImageTarget::Jpg) => args.extend(strings(&["-frames:v", "1", "-update", "1", "-vf", "format=rgb24", "-q:v", "2"])),
        Target::Image(ImageTarget::Png) => args.extend(strings(&["-frames:v", "1", "-update", "1"])),
        Target::Image(ImageTarget::Webp) => args.extend(strings(&["-frames:v", "1", "-update", "1", "-c:v", "libwebp", "-q:v", "90"])),
    }
    args.push(text(dest));
    args
}

/// Seconds from the `HH:MM:SS.ss` that follows `label` in `line`.
fn clock_after(line: &str, label: &str) -> Option<f64> {
    let rest = line.split_once(label)?.1.trim_start();
    let clock = rest.split(|c: char| !(c.is_ascii_digit() || c == ':' || c == '.')).next()?;
    let mut parts = clock.split(':');
    let (hours, minutes, seconds) = (parts.next()?, parts.next()?, parts.next()?);
    Some(hours.parse::<f64>().ok()? * 3600.0 + minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?)
}

/// Tracks ffmpeg output and yields a whole percentage (0..=99) each time it changes.
#[derive(Default)]
pub struct Progress {
    duration: Option<f64>,
    last: Option<u8>,
}

impl Progress {
    pub fn feed(&mut self, line: &str) -> Option<u8> {
        if let Some(duration) = clock_after(line, "Duration:") {
            self.duration = Some(duration);
        }
        let current = clock_after(line, "out_time=")?;
        let duration = self.duration.filter(|d| *d > 0.0)?;
        let percent = (current / duration * 100.0).clamp(0.0, 99.0) as u8;
        (self.last != Some(percent)).then(|| {
            self.last = Some(percent);
            percent
        })
    }
}

/// Result of one conversion.
pub struct Converted {
    /// 0 on success, `proc::CANCELLED` when cancelled, otherwise ffmpeg's exit code.
    pub code: i32,
    pub dest: PathBuf,
    /// The encoder that produced the file (the CPU when a hardware attempt failed).
    pub encoder: Encoder,
}

/// Runs `encode` with `preferred`; when a hardware encoder fails, runs it again on the CPU.
fn with_cpu_fallback(preferred: Encoder, job: &Job, mut encode: impl FnMut(Encoder) -> std::io::Result<i32>) -> std::io::Result<(i32, Encoder)> {
    let code = encode(preferred)?;
    if code == 0 || code == CANCELLED || preferred == Encoder::Cpu || job.cancelled() {
        return Ok((code, preferred));
    }
    Ok((encode(Encoder::Cpu)?, Encoder::Cpu))
}

/// Converts one file. `encoder` only matters for MP4 output.
pub fn convert(source: &Path, target: Target, ffmpeg: &str, encoder: Encoder, job: &Job, on_progress: &mut dyn FnMut(u8)) -> std::io::Result<Converted> {
    let dest = reserve_output(source, target)?;
    let mp4 = matches!(target, Target::Video { target: VideoTarget::Mp4, .. });
    let result = with_cpu_fallback(if mp4 { encoder } else { Encoder::Cpu }, job, |encoder| {
        let mut progress = Progress::default();
        proc::run(ffmpeg, &build_args(source, &dest, target, encoder), job, |line| {
            if let Some(percent) = progress.feed(line) {
                on_progress(percent);
            }
        })
    });
    // Anything but success (failure, cancel, ffmpeg that could not start) leaves the reserved
    // or half-written file behind; it would look like a finished conversion.
    if !matches!(result, Ok((0, _))) {
        let _ = std::fs::remove_file(&dest);
    }
    let (code, encoder) = result?;
    Ok(Converted { code, dest, encoder })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MP4: Target = Target::Video { target: VideoTarget::Mp4, height: 1080, quality: 1 };
    const JPG: Target = Target::Image(ImageTarget::Jpg);

    fn temp(name: &str, files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xdownload2_conv_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        root
    }

    fn names(files: &[PathBuf]) -> Vec<String> {
        files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
        &args[args.iter().position(|a| a == flag).unwrap() + 1]
    }

    #[test]
    fn collects_folder_videos_without_generated_outputs_or_nested_files() {
        let root = temp("videos", &["b.mov", "a.MP4", "done_h264_1080p.mp4", "done_h264_720p-2.mp4", "x_h264_src.mp4", "my_h264_notes.mp4", "nested/c.mkv", "note.txt"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], MP4, false)), ["a.MP4", "b.mov", "my_h264_notes.mp4"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], MP4, true)), ["a.MP4", "b.mov", "my_h264_notes.mp4", "c.mkv"]);
        // A file picked by hand is taken even when it is an earlier output.
        let picked = [root.join("done_h264_1080p.mp4"), root.join("note.txt"), root.join("missing.mp4")];
        assert_eq!(names(&collect_media_files(&picked, MP4, false)), ["done_h264_1080p.mp4"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn images_already_in_the_target_format_are_skipped() {
        let root = temp("images", &["a.png", "b.jpeg", "c.jpg", "d.webp", "clip.mp4"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], JPG, false)), ["a.png", "b.jpeg", "d.webp"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], Target::Image(ImageTarget::Png), false)), ["b.jpeg", "c.jpg", "d.webp"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], Target::Image(ImageTarget::Webp), false)), ["a.png", "b.jpeg", "c.jpg"]);
        assert_eq!(is_video_file(&root.join("clip.mp4")), Some(true));
        assert_eq!(is_video_file(&root.join("a.PNG")), Some(false));
        assert_eq!(is_video_file(&root.join("note.txt")), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn output_paths_are_unique_and_keep_originals() {
        let root = temp("outputs", &["clip.mp4", "clip_h264_1080p.mp4", "photo.jpeg", "photo.jpg", "movie.mkv"]);
        // The first name that does not exist yet.
        let name = |source: &str, target| names(&[output_candidates(&root.join(source), target).find(|path| !path.exists()).unwrap()])[0].clone();
        assert_eq!(name("clip.mp4", MP4), "clip_h264_1080p-2.mp4");
        assert_eq!(name("clip.mp4", Target::Video { target: VideoTarget::Mp4, height: 720, quality: 0 }), "clip_h264_720p.mp4");
        assert_eq!(name("clip.mp4", Target::Video { target: VideoTarget::Mp4, height: 0, quality: 0 }), "clip_h264_src.mp4");
        assert_eq!(name("movie.mkv", MP4), "movie.mp4");
        assert_eq!(name("movie.mkv", Target::Video { target: VideoTarget::Mp3, height: 0, quality: 0 }), "movie.mp3");
        assert_eq!(name("movie.mkv", Target::Video { target: VideoTarget::Gif, height: 0, quality: 0 }), "movie.gif");
        assert_eq!(name("photo.jpeg", JPG), "photo-2.jpg");
        assert_eq!(name("photo.jpeg", Target::Image(ImageTarget::Webp)), "photo.webp");
        // Reserving creates the file, so the next conversion with the same name moves on.
        let first = reserve_output(&root.join("movie.mkv"), MP4).unwrap();
        let second = reserve_output(&root.join("movie.mkv"), MP4).unwrap();
        assert_eq!(names(&[first.clone(), second]), ["movie.mp4", "movie-2.mp4"]);
        // A conversion that cannot even start gives its reserved name back.
        std::fs::remove_file(&first).unwrap();
        assert!(convert(&root.join("movie.mkv"), MP4, "xdownload2-no-such-ffmpeg", Encoder::Cpu, &Job::default(), &mut |_| {}).is_err());
        assert!(!first.exists(), "no empty output is left behind");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mp4_args_follow_height_quality_and_encoder() {
        let args = |height, quality, encoder| build_args(Path::new("in.mov"), Path::new("out.mp4"), Target::Video { target: VideoTarget::Mp4, height, quality }, encoder);
        let cpu = args(1080, 1, Encoder::Cpu);
        assert_eq!((after(&cpu, "-c:v"), after(&cpu, "-crf"), after(&cpu, "-vf")), ("libx264", "23", "scale=-2:min(1080\\,ih)"));
        assert_eq!((after(&cpu, "-c:a"), after(&cpu, "-movflags"), after(&cpu, "-i"), after(&cpu, "-progress")), ("aac", "+faststart", "in.mov", "pipe:1"));
        assert!(cpu.contains(&"0:a:0?".to_owned()), "a video without audio still converts");
        assert_eq!(cpu.last().unwrap(), "out.mp4");
        assert_eq!(after(&args(720, 0, Encoder::Cpu), "-crf"), "20");
        assert_eq!(after(&args(0, 2, Encoder::Cpu), "-crf"), "28");
        assert_eq!(after(&args(0, 9, Encoder::Cpu), "-vf"), "scale=trunc(iw/2)*2:trunc(ih/2)*2");
        let nvenc = args(1080, 1, Encoder::Nvenc);
        assert_eq!((after(&nvenc, "-c:v"), after(&nvenc, "-cq"), after(&nvenc, "-rc")), ("h264_nvenc", "30", "vbr"));
        assert!(!nvenc.contains(&"-crf".to_owned()));
        let qsv = args(1080, 1, Encoder::Qsv);
        assert_eq!((after(&qsv, "-c:v"), after(&qsv, "-global_quality"), after(&qsv, "-pix_fmt")), ("h264_qsv", "26", "nv12"));
        let amf = args(1080, 1, Encoder::Amf);
        assert_eq!((after(&amf, "-c:v"), after(&amf, "-qp_i"), after(&amf, "-rc")), ("h264_amf", "26", "cqp"));
    }

    #[test]
    fn other_targets_have_their_own_args() {
        let args = |target, dest: &str| build_args(Path::new("in.x"), Path::new(dest), target, Encoder::Nvenc);
        let mp3 = args(Target::Video { target: VideoTarget::Mp3, height: 0, quality: 0 }, "o.mp3");
        assert!(mp3.contains(&"-vn".to_owned()) && after(&mp3, "-c:a") == "libmp3lame" && !mp3.contains(&"h264_nvenc".to_owned()));
        let gif = args(Target::Video { target: VideoTarget::Gif, height: 0, quality: 0 }, "o.gif");
        assert!(after(&gif, "-vf").contains("palettegen") && after(&gif, "-loop") == "0");
        let jpg = args(JPG, "o.jpg");
        assert_eq!((after(&jpg, "-vf"), after(&jpg, "-q:v"), after(&jpg, "-frames:v")), ("format=rgb24", "2", "1"));
        let webp = args(Target::Image(ImageTarget::Webp), "o.webp");
        assert_eq!((after(&webp, "-c:v"), after(&webp, "-update")), ("libwebp", "1"));
        let png = args(Target::Image(ImageTarget::Png), "o.png");
        assert_eq!((after(&png, "-frames:v"), after(&png, "-update"), png.last().unwrap().as_str()), ("1", "1", "o.png"));
        assert_eq!(after(&jpg, "-update"), "1");
    }

    #[test]
    fn a_failed_hardware_encode_is_repeated_on_the_cpu() {
        let run = |preferred, codes: &[i32], cancel: bool| {
            let job = Job::default();
            let mut used = Vec::new();
            let mut codes = codes.iter().copied();
            let result = with_cpu_fallback(preferred, &job, |encoder| {
                used.push(encoder);
                if cancel {
                    job.cancel();
                }
                Ok(codes.next().unwrap())
            }).unwrap();
            (result, used)
        };
        assert_eq!(run(Encoder::Nvenc, &[0], false), ((0, Encoder::Nvenc), vec![Encoder::Nvenc]));
        assert_eq!(run(Encoder::Nvenc, &[1, 0], false), ((0, Encoder::Cpu), vec![Encoder::Nvenc, Encoder::Cpu]));
        assert_eq!(run(Encoder::Cpu, &[1], false), ((1, Encoder::Cpu), vec![Encoder::Cpu]));
        assert_eq!(run(Encoder::Amf, &[CANCELLED], true).1, vec![Encoder::Amf], "a cancel is not retried");
    }

    #[test]
    fn progress_needs_a_duration_and_reports_each_percent_once() {
        let mut progress = Progress::default();
        assert_eq!(progress.feed("out_time=00:00:05.000000"), None, "no duration yet");
        assert_eq!(progress.feed("  Duration: N/A, start: 0.000000"), None);
        assert_eq!(progress.feed("  Duration: 00:01:40.00, start: 0.000000, bitrate: 900 kb/s"), None);
        assert_eq!(progress.feed("out_time=00:00:25.500000"), Some(25));
        assert_eq!(progress.feed("out_time=00:00:25.900000"), None, "same percent");
        assert_eq!(progress.feed("out_time=N/A"), None);
        assert_eq!(progress.feed("out_time=01:00:00.000000"), Some(99), "never 100 before ffmpeg exits");
        assert_eq!(progress.feed("out_time_ms=1000"), None);
    }
}
