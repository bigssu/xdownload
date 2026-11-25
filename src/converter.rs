//! Local media conversion: ffmpeg turns videos and images into widely playable formats.
//! The original is kept and the new file is written next to it.
use crate::proc::{self, Job};
use std::path::{Path, PathBuf};

pub const VIDEO_EXTS: [&str; 12] = ["3gp", "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mts", "ts", "webm", "wmv"];
/// `.jpg` is absent on purpose: it is already the target format.
pub const IMAGE_EXTS: [&str; 11] = ["avif", "bmp", "gif", "heic", "heif", "jfif", "jpeg", "png", "tif", "tiff", "webp"];
pub const VIDEO_SUFFIX: &str = "_h264_1080p";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Video,
    Image,
}

impl Kind {
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::Video => &VIDEO_EXTS,
            Kind::Image => &IMAGE_EXTS,
        }
    }

    fn accepts(self, path: &Path) -> bool {
        extension(path).is_some_and(|ext| self.extensions().contains(&ext.as_str()))
    }
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_lowercase())
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A video this module wrote earlier: `name_h264_1080p` or `name_h264_1080p-2`.
fn is_generated_video(path: &Path) -> bool {
    let stem = stem(path).to_lowercase();
    if stem.ends_with(VIDEO_SUFFIX) {
        return true;
    }
    stem.rsplit_once('-').is_some_and(|(name, number)| {
        name.ends_with(VIDEO_SUFFIX) && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    })
}

/// Files to convert from a list of files and folders.
///
/// A folder contributes only the files directly inside it. Outputs land in the same folder,
/// so earlier video outputs are skipped rather than converted again.
pub fn collect_media_files(sources: &[PathBuf], kind: Kind) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for source in sources {
        if source.is_file() {
            if kind.accepts(source) {
                files.push(source.clone());
            }
            continue;
        }
        let Ok(entries) = std::fs::read_dir(source) else { continue };
        let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path())
            .filter(|path| path.is_file() && kind.accepts(path) && !(kind == Kind::Video && is_generated_video(path)))
            .collect();
        children.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()));
        files.extend(children);
    }
    files
}

/// `path`, or `name-2.ext`, `name-3.ext`, ... when it already exists.
fn unique_path(path: PathBuf) -> std::io::Result<PathBuf> {
    if !path.exists() {
        return Ok(path);
    }
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    (2..10000).map(|n| path.with_file_name(format!("{}-{n}.{ext}", stem(&path)))).find(|candidate| !candidate.exists())
        .ok_or_else(|| std::io::Error::other(format!("no free output name for {}", path.display())))
}

pub fn video_output_path(source: &Path) -> std::io::Result<PathBuf> {
    if extension(source).as_deref() == Some("mp4") {
        return unique_path(source.with_file_name(format!("{}{VIDEO_SUFFIX}.mp4", stem(source))));
    }
    unique_path(source.with_extension("mp4"))
}

/// `None` for a file that is already a `.jpg`.
pub fn image_output_path(source: &Path) -> std::io::Result<Option<PathBuf>> {
    if extension(source).as_deref() == Some("jpg") {
        return Ok(None);
    }
    unique_path(source.with_extension("jpg")).map(Some)
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// ffmpeg arguments (without the program): MP4 / H.264 / AAC, at most 1080p.
pub fn build_video_args(source: &Path, dest: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["-y", "-hide_banner", "-progress", "pipe:1", "-nostats", "-i"].map(String::from).to_vec();
    args.push(text(source));
    args.extend([
        "-map", "0:v:0", "-map", "0:a:0?", "-vf", "scale=-2:min(1080\\,ih)", "-c:v", "libx264", "-preset", "medium",
        "-crf", "23", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart",
    ].map(String::from));
    args.push(text(dest));
    args
}

/// ffmpeg arguments (without the program): one high-quality JPG frame.
pub fn build_image_args(source: &Path, dest: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["-y", "-hide_banner", "-i"].map(String::from).to_vec();
    args.push(text(source));
    args.extend(["-frames:v", "1", "-vf", "format=rgb24", "-q:v", "2"].map(String::from));
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

/// Converts one video. 0 on success, `proc::CANCELLED` when cancelled, otherwise ffmpeg's exit code.
pub fn convert_video(source: &Path, ffmpeg: &str, job: &Job, on_progress: &mut dyn FnMut(u8)) -> std::io::Result<i32> {
    let dest = video_output_path(source)?;
    let mut progress = Progress::default();
    let code = proc::run(ffmpeg, &build_video_args(source, &dest), job, |line| {
        if let Some(percent) = progress.feed(line) {
            on_progress(percent);
        }
    })?;
    discard_partial(code, &dest);
    Ok(code)
}

/// Converts one image; a `.jpg` source is left alone and counts as done.
pub fn convert_image(source: &Path, ffmpeg: &str, job: &Job) -> std::io::Result<i32> {
    let Some(dest) = image_output_path(source)? else { return Ok(0) };
    let code = proc::run(ffmpeg, &build_image_args(source, &dest), job, |_| {})?;
    discard_partial(code, &dest);
    Ok(code)
}

/// A failed or cancelled run leaves a broken output that would look like a finished file.
fn discard_partial(code: i32, dest: &Path) {
    if code != 0 {
        let _ = std::fs::remove_file(dest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn collects_folder_videos_without_generated_outputs_or_nested_files() {
        let root = temp("videos", &["b.mov", "a.MP4", "done_h264_1080p.mp4", "done_h264_1080p-2.mp4", "nested/c.mkv", "note.txt"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], Kind::Video)), ["a.MP4", "b.mov"]);
        // A file picked by hand is taken even when it is an earlier output.
        let picked = [root.join("done_h264_1080p.mp4"), root.join("note.txt"), root.join("missing.mp4")];
        assert_eq!(names(&collect_media_files(&picked, Kind::Video)), ["done_h264_1080p.mp4"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn collects_only_images_that_need_jpg_outputs() {
        let root = temp("images", &["a.png", "b.jpeg", "c.jpg"]);
        assert_eq!(names(&collect_media_files(&[root.clone()], Kind::Image)), ["a.png", "b.jpeg"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn output_paths_are_unique_and_keep_originals() {
        let root = temp("outputs", &["clip.mp4", "clip_h264_1080p.mp4", "photo.jpeg", "photo.jpg", "movie.mkv"]);
        assert_eq!(names(&[video_output_path(&root.join("clip.mp4")).unwrap()]), ["clip_h264_1080p-2.mp4"]);
        assert_eq!(names(&[video_output_path(&root.join("movie.mkv")).unwrap()]), ["movie.mp4"]);
        assert_eq!(names(&[image_output_path(&root.join("photo.jpeg")).unwrap().unwrap()]), ["photo-2.jpg"]);
        assert_eq!(image_output_path(&root.join("photo.jpg")).unwrap(), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn video_args_enforce_requested_compatibility_format() {
        let args = build_video_args(Path::new("in.mov"), Path::new("out.mp4"));
        for wanted in ["libx264", "aac", "scale=-2:min(1080\\,ih)", "pipe:1", "+faststart", "0:a:0?"] {
            assert!(args.contains(&wanted.to_owned()), "{wanted}");
        }
        assert_eq!(args[args.iter().position(|a| a == "-i").unwrap() + 1], "in.mov");
        assert_eq!(args.last().unwrap(), "out.mp4");
    }

    #[test]
    fn image_args_write_one_high_quality_jpg_frame() {
        let args = build_image_args(Path::new("in.png"), Path::new("out.jpg"));
        assert!(args.contains(&"format=rgb24".to_owned()));
        assert_eq!(args[args.iter().position(|a| a == "-q:v").unwrap() + 1], "2");
        assert_eq!(args[args.iter().position(|a| a == "-frames:v").unwrap() + 1], "1");
        assert_eq!(args.last().unwrap(), "out.jpg");
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
