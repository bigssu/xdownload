//! The work queue: downloads and conversions waiting, running and finished.
//!
//! The queue itself is plain data owned by the UI thread. Every running item has its own
//! thread and `Job`; the thread reports through a channel of `Msg`. A retry gets a new id, so
//! a late message from the cancelled run can never be mistaken for the new one.
use crate::converter::{self, Encoder, Target};
use crate::downloader::{self, Event, Options};
use crate::proc::{Job, CANCELLED};
use crate::{binaries, settings};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, PartialEq, Debug)]
pub enum Work {
    Download { url: String, options: Options },
    Convert { source: PathBuf, target: Target },
}

/// Items of one class share a limit on how many run at once.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Class {
    Download,
    Video,
    Image,
}

impl Work {
    pub fn class(&self) -> Class {
        match self {
            Work::Download { .. } => Class::Download,
            Work::Convert { target, .. } if target.is_video() => Class::Video,
            Work::Convert { .. } => Class::Image,
        }
    }
}

/// How many items of each class may run at once.
#[derive(Clone, Copy)]
pub struct Limits {
    pub download: usize,
    pub video: usize,
    pub image: usize,
}

impl Limits {
    /// Videos: one at a time on the CPU (libx264 already uses every core), two on a graphics
    /// card. Images: one per core, since each ffmpeg uses a single core for a still.
    pub fn new(parallel_downloads: usize, gpu: bool) -> Self {
        // Asked once: this runs every frame.
        static CORES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        let cores = *CORES.get_or_init(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
        Self { download: parallel_downloads.max(1), video: if gpu { 2 } else { 1 }, image: cores.clamp(1, 8) }
    }

    fn of(&self, class: Class) -> usize {
        match class {
            Class::Download => self.download,
            Class::Video => self.video,
            Class::Image => self.image,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Waiting,
    Running,
    Done,
    Failed,
    Cancelled,
}

/// The small line under an item's title.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum Note {
    #[default]
    None,
    /// Fetching yt-dlp / ffmpeg.
    Preparing,
    /// Updating yt-dlp before the second attempt.
    Retrying,
    Speed { speed: String, eta: String },
    Encoder(&'static str),
    Error(String),
}

pub struct Item {
    pub id: u64,
    pub work: Work,
    pub title: String,
    pub state: State,
    /// 0..=1
    pub progress: f32,
    pub note: Note,
    /// The newest finished file (a playlist finishes several).
    pub file: Option<PathBuf>,
    job: Option<Arc<Job>>,
}

impl Item {
    pub fn finished(&self) -> bool {
        !matches!(self.state, State::Waiting | State::Running)
    }

    /// True once a cancel was asked for but the worker has not stopped yet.
    pub fn cancelling(&self) -> bool {
        self.state == State::Running && self.job.as_ref().is_some_and(|job| job.cancelled())
    }
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, PartialEq)]
pub enum Update {
    Title(String),
    Progress(f32),
    Note(Note),
    /// A file was completed; `title` is the video's title for a download.
    File { path: PathBuf, title: String },
    Finished(Outcome),
}

#[derive(Debug, PartialEq)]
pub struct Msg {
    pub id: u64,
    pub update: Update,
}

/// What `Queue::apply` tells the caller about.
pub enum Applied<'a> {
    Nothing,
    /// A download finished this file (with the video's title).
    Downloaded(&'a Item, PathBuf, String),
    /// An item reached Done, Failed or Cancelled.
    Finished,
}

/// What makes two pieces of work the same: a link, whatever its options (both yt-dlp runs
/// would write the same files), or a source file with its target.
#[derive(PartialEq, Eq, Hash)]
enum Key {
    Link(String),
    File(PathBuf, Target),
}

impl Key {
    fn of(work: &Work) -> Self {
        match work {
            Work::Download { url, .. } => Key::Link(url.clone()),
            Work::Convert { source, target } => Key::File(source.clone(), *target),
        }
    }
}

#[derive(Default)]
pub struct Queue {
    pub items: Vec<Item>,
    next_id: u64,
}

fn default_title(work: &Work) -> String {
    match work {
        Work::Download { url, .. } => url.clone(),
        Work::Convert { source, .. } => source.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
    }
}

impl Queue {
    /// Sample items in every state, for `--screenshot`.
    pub fn demo(options: Options) -> Self {
        let mut queue = Self::default();
        let samples = [
            ("Sample video title one", State::Running, 0.42, Note::Speed { speed: "5.31MiB/s".into(), eta: "00:12".into() }),
            ("Sample video title two", State::Running, 0.08, Note::Speed { speed: "2.10MiB/s".into(), eta: "01:05".into() }),
            ("https://youtu.be/waiting", State::Waiting, 0.0, Note::None),
            ("Finished video", State::Done, 1.0, Note::None),
            ("Private video", State::Failed, 0.0, Note::Error("exit 1".into())),
        ];
        for (n, (title, state, progress, note)) in samples.into_iter().enumerate() {
            let id = queue.add(Work::Download { url: format!("https://youtu.be/demo{n}"), options: options.clone() }).unwrap_or_default();
            if let Some(item) = queue.get_mut(id) {
                (item.title, item.state, item.progress, item.note) = (title.into(), state, progress, note);
            }
        }
        let target = Target::Video { target: converter::VideoTarget::Mp4, height: 1080, quality: 1 };
        if let Some(item) = queue.add(Work::Convert { source: "holiday.mov".into(), target }).and_then(|id| queue.get_mut(id)) {
            (item.state, item.progress, item.note) = (State::Running, 0.67, Note::Encoder(Encoder::Nvenc.label()));
        }
        queue
    }

    /// Adds work to the end. Work already waiting or running is not added twice, and neither
    /// is a second download of the same link with other options: both yt-dlp runs would write
    /// the same intermediate and final file names.
    pub fn add(&mut self, work: Work) -> Option<u64> {
        (self.add_all([work]) == 1).then_some(self.next_id)
    }

    /// `add` for many at once; returns how many were added. The waiting and running work is
    /// collected once, so dropping thousands of files stays fast.
    pub fn add_all(&mut self, works: impl IntoIterator<Item = Work>) -> usize {
        let mut active: HashSet<Key> = self.items.iter().filter(|item| !item.finished()).map(|item| Key::of(&item.work)).collect();
        let mut added = 0;
        for work in works {
            if active.insert(Key::of(&work)) {
                self.next_id += 1;
                self.items.push(Item { id: self.next_id, title: default_title(&work), work, state: State::Waiting, progress: 0.0, note: Note::None, file: None, job: None });
                added += 1;
            }
        }
        added
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn count(&self, state: State) -> usize {
        self.items.iter().filter(|item| item.state == state).count()
    }

    /// Anything waiting or running.
    pub fn active(&self) -> bool {
        self.items.iter().any(|item| !item.finished())
    }

    /// Marks the next waiting items as running, within each class's limit, and returns what to
    /// start: the caller spawns one worker per entry.
    pub fn start_next(&mut self, limits: Limits) -> Vec<(u64, Work, Arc<Job>)> {
        let mut started = Vec::new();
        for class in [Class::Download, Class::Video, Class::Image] {
            let mut free = limits.of(class).saturating_sub(self.items.iter().filter(|item| item.state == State::Running && item.work.class() == class).count());
            for item in self.items.iter_mut().filter(|item| item.state == State::Waiting && item.work.class() == class) {
                if free == 0 {
                    break;
                }
                free -= 1;
                let job = Arc::new(Job::default());
                item.state = State::Running;
                item.job = Some(job.clone());
                started.push((item.id, item.work.clone(), job));
            }
        }
        started
    }

    /// Applies a worker's message. Messages for removed or already finished items are dropped.
    pub fn apply(&mut self, msg: Msg) -> Applied<'_> {
        let Some(item) = self.get_mut(msg.id).filter(|item| item.state == State::Running) else { return Applied::Nothing };
        match msg.update {
            Update::Title(title) => item.title = title,
            Update::Progress(fraction) => item.progress = fraction.clamp(0.0, 1.0),
            Update::Note(note) => item.note = note,
            Update::File { path, title } => {
                item.file = Some(path.clone());
                if matches!(item.work, Work::Download { .. }) {
                    return Applied::Downloaded(item, path, title);
                }
            }
            Update::Finished(outcome) => {
                item.job = None;
                match outcome {
                    Outcome::Done => (item.state, item.progress) = (State::Done, 1.0),
                    Outcome::Cancelled => (item.state, item.note) = (State::Cancelled, Note::None),
                    Outcome::Failed(error) => (item.state, item.note) = (State::Failed, Note::Error(error)),
                }
                return Applied::Finished;
            }
        }
        Applied::Nothing
    }

    /// Cancels one item. A waiting item stops at once; a running one keeps its slot until its
    /// worker reports back, so its process is really gone before another starts.
    pub fn cancel(&mut self, id: u64) {
        let Some(item) = self.get_mut(id) else { return };
        match item.state {
            State::Waiting => item.state = State::Cancelled,
            State::Running => {
                if let Some(job) = item.job.clone() {
                    // Killing a process tree can take a moment; keep it off the UI thread.
                    std::thread::spawn(move || job.cancel());
                }
            }
            _ => {}
        }
    }

    pub fn cancel_all(&mut self) {
        let ids: Vec<u64> = self.items.iter().filter(|item| !item.finished()).map(|item| item.id).collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Stops every process now, on this thread (the window is closing).
    pub fn cancel_all_blocking(&mut self) {
        for item in &mut self.items {
            match item.state {
                State::Waiting => item.state = State::Cancelled,
                State::Running => {
                    if let Some(job) = &item.job {
                        job.cancel();
                    }
                }
                _ => {}
            }
        }
    }

    /// Queues a failed or cancelled item again, in place, under a new id.
    pub fn retry(&mut self, id: u64) {
        self.next_id += 1;
        let next_id = self.next_id;
        if let Some(item) = self.get_mut(id).filter(|item| matches!(item.state, State::Failed | State::Cancelled)) {
            (item.id, item.state, item.progress, item.note, item.file) = (next_id, State::Waiting, 0.0, Note::None, None);
        }
    }

    /// Removes an item that is not running.
    pub fn remove(&mut self, id: u64) {
        self.items.retain(|item| item.id != id || item.state == State::Running);
    }

    pub fn clear_finished(&mut self) {
        self.items.retain(|item| !item.finished());
    }
}

/// The body of one worker thread: prepares the tools, runs the work and reports through `send`.
/// Always ends with exactly one `Update::Finished`.
pub fn run(work: Work, job: &Job, use_gpu: bool, send: &dyn Fn(Update)) {
    if job.cancelled() {
        return send(Update::Finished(Outcome::Cancelled));
    }
    let outcome = match run_inner(&work, job, use_gpu, send) {
        _ if job.cancelled() => Outcome::Cancelled,
        Ok(0) => Outcome::Done,
        Ok(CANCELLED) => Outcome::Cancelled,
        Ok(code) => Outcome::Failed(format!("exit {code}")),
        Err(error) => Outcome::Failed(error),
    };
    send(Update::Finished(outcome));
}

fn run_inner(work: &Work, job: &Job, use_gpu: bool, send: &dyn Fn(Update)) -> Result<i32, String> {
    send(Update::Note(Note::Preparing));
    let mut prep_progress = |fraction: f32| send(Update::Progress(fraction));
    match work {
        Work::Download { url, options } => {
            let ytdlp = binaries::ensure_ytdlp(job, &mut prep_progress)?;
            let ffmpeg = binaries::ensure_ffmpeg(job, &mut prep_progress)?;
            send(Update::Progress(0.0));
            send(Update::Note(Note::None));
            let mut playlist = None;
            // The reason yt-dlp printed, when it printed one, says more than its exit code. Shared
            // with the retry callback, which forgets the first attempt's reason.
            let reason = std::cell::RefCell::new(String::new());
            // The video being downloaded: its streams (video and audio arrive one after the
            // other, each 0..100%) and its title. Set again for every playlist video and retry.
            let (mut streams, mut title) = (String::new(), String::new());
            let on_retry = &mut || {
                reason.borrow_mut().clear();
                send(Update::Note(Note::Retrying));
            };
            let code = downloader::download_with_auto_update(url, options, &ytdlp, &ffmpeg, job, on_retry, &mut |event| match event {
                Event::Progress { stream, percent, speed, eta } => {
                    // Subtitles and thumbnails are not among the streams and do not move the bar.
                    if let Some(fraction) = download_progress(&streams, stream, percent) {
                        send(Update::Progress(fraction));
                        send(Update::Note(Note::Speed { speed: speed.to_owned(), eta: eta.to_owned() }));
                    }
                }
                Event::Item { index, total } => playlist = Some((index, total)),
                Event::Title { streams: ids, title: name } => {
                    streams = ids.to_owned();
                    title = name.to_owned();
                    send(Update::Title(match playlist {
                        Some((index, total)) => format!("({index}/{total}) {name}"),
                        None => name.to_owned(),
                    }));
                }
                Event::File(path) => send(Update::File { path: PathBuf::from(path), title: title.clone() }),
                Event::Error(text) => *reason.borrow_mut() = text.to_owned(),
            }).map_err(|error| error.to_string())?;
            let reason = reason.into_inner();
            if code == 0 || code == CANCELLED || reason.is_empty() { Ok(code) } else { Err(reason) }
        }
        Work::Convert { source, target } => {
            let ffmpeg = binaries::ensure_ffmpeg(job, &mut prep_progress)?;
            send(Update::Progress(0.0));
            let encoder = if use_gpu && target.is_video() { converter::detect_encoder(&ffmpeg) } else { Encoder::Cpu };
            send(Update::Note(if matches!(target, Target::Video { target: converter::VideoTarget::Mp4, .. }) { Note::Encoder(encoder.label()) } else { Note::None }));
            let converted = converter::convert(source, *target, &ffmpeg, encoder, job, &mut |percent| send(Update::Progress(percent as f32 / 100.0))).map_err(|error| error.to_string())?;
            if converted.code == 0 {
                if converted.encoder != encoder {
                    send(Update::Note(Note::Encoder(converted.encoder.label())));
                }
                send(Update::File { path: converted.dest, title: String::new() });
            }
            Ok(converted.code)
        }
    }
}

/// Overall progress (0..=1) of a download whose streams (`137+140`) arrive one after the
/// other, from a progress line about `stream`. A line about all streams at once (ffmpeg
/// fetching a live or HLS video in one pass reports `137+140`) is the whole download.
/// `None` for anything else, such as subtitles.
fn download_progress(streams: &str, stream: &str, percent: f32) -> Option<f32> {
    if stream == streams {
        return Some(percent / 100.0);
    }
    let count = streams.split('+').count();
    let index = streams.split('+').position(|known| known == stream)?;
    Some((index as f32 + percent / 100.0) / count as f32)
}

/// History entry for a file a download finished.
pub fn history_entry(item: &Item, file: &std::path::Path, title: String) -> Option<settings::Entry> {
    let Work::Download { url, options } = &item.work else { return None };
    Some(settings::Entry {
        url: url.clone(),
        title,
        file: file.to_string_lossy().into_owned(),
        format: options.format.name().into(),
        quality: options.quality_name(),
        path: options.out_dir.clone(),
        when: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{ImageTarget, VideoTarget};
    use crate::downloader::Format;

    fn download(url: &str) -> Work {
        Work::Download { url: url.into(), options: Options { format: Format::Mp4, height: 720, subs: false, embed_info: false, playlist: false, out_dir: "C:/out".into() } }
    }

    fn image(name: &str) -> Work {
        Work::Convert { source: name.into(), target: Target::Image(ImageTarget::Jpg) }
    }

    fn video(name: &str) -> Work {
        Work::Convert { source: name.into(), target: Target::Video { target: VideoTarget::Mp4, height: 0, quality: 1 } }
    }

    const LIMITS: Limits = Limits { download: 2, video: 1, image: 3 };

    fn finish(queue: &mut Queue, id: u64, outcome: Outcome) {
        assert!(matches!(queue.apply(Msg { id, update: Update::Finished(outcome) }), Applied::Finished));
    }

    #[test]
    fn items_start_in_order_within_each_class_limit() {
        let mut queue = Queue::default();
        let ids: Vec<u64> = ["a", "b", "c"].iter().map(|url| queue.add(download(url)).unwrap()).collect();
        let videos = [queue.add(video("v1.mov")).unwrap(), queue.add(video("v2.mov")).unwrap()];
        for n in 0..4 {
            queue.add(image(&format!("p{n}.png")));
        }
        assert_eq!(queue.add(download("b")), None, "the same link is not queued twice while active");
        let Work::Download { url, mut options } = download("b") else { unreachable!() };
        options.height = 1080;
        assert_eq!(queue.add(Work::Download { url, options }), None, "not even with other options: the file names would clash");
        let started: Vec<u64> = queue.start_next(LIMITS).into_iter().map(|(id, _, _)| id).collect();
        assert_eq!(started, [ids[0], ids[1], videos[0], 6, 7, 8]);
        assert!(queue.start_next(LIMITS).is_empty(), "every limit is used up");
        assert_eq!((queue.count(State::Running), queue.count(State::Waiting)), (6, 3));
        finish(&mut queue, ids[0], Outcome::Done);
        let started: Vec<u64> = queue.start_next(LIMITS).into_iter().map(|(id, _, _)| id).collect();
        assert_eq!(started, [ids[2]], "a freed slot goes to the next waiting item of that class");
        assert!(queue.add(download("a")).is_some(), "a finished link can be queued again");
        assert_eq!(Limits::new(0, true).download, 1);
        assert_eq!((Limits::new(3, true).video, Limits::new(3, false).video), (2, 1));
    }

    #[test]
    fn messages_update_the_item_and_report_files_and_completion() {
        let mut queue = Queue::default();
        let id = queue.add(download("a")).unwrap();
        assert!(matches!(queue.apply(Msg { id, update: Update::Progress(0.5) }), Applied::Nothing), "not running yet");
        assert_eq!(queue.items[0].progress, 0.0);
        queue.start_next(LIMITS);
        queue.apply(Msg { id, update: Update::Title("Song".into()) });
        queue.apply(Msg { id, update: Update::Progress(7.0) });
        queue.apply(Msg { id, update: Update::Note(Note::Speed { speed: "1MiB/s".into(), eta: "00:03".into() }) });
        assert_eq!((queue.items[0].title.as_str(), queue.items[0].progress), ("Song", 1.0));
        let update = Update::File { path: "C:/out/Song ⧸ live [abcdefghijk].mp4".into(), title: "Song / live".into() };
        let Applied::Downloaded(item, file, title) = queue.apply(Msg { id, update }) else { panic!("a finished download file is reported") };
        let entry = history_entry(item, &file, title).unwrap();
        assert_eq!((entry.title.as_str(), entry.format.as_str(), entry.quality.as_str(), entry.path.as_str(), entry.url.as_str()), ("Song / live", "mp4", "720p", "C:/out", "a"), "the title as YouTube has it, not as the file name had to spell it");
        assert!(entry.file.ends_with("Song ⧸ live [abcdefghijk].mp4"), "the history keeps the real file name");
        finish(&mut queue, id, Outcome::Failed("exit 1".into()));
        assert_eq!((queue.items[0].state, &queue.items[0].note), (State::Failed, &Note::Error("exit 1".into())));
        assert!(matches!(queue.apply(Msg { id, update: Update::Progress(0.1) }), Applied::Nothing), "late messages are dropped");
        assert!(!queue.active());
        // A conversion's file is kept on the item but is not a download for the history.
        let id = queue.add(image("p.png")).unwrap();
        queue.start_next(LIMITS);
        assert!(matches!(queue.apply(Msg { id, update: Update::File { path: "p.jpg".into(), title: String::new() } }), Applied::Nothing));
        assert_eq!(queue.items[1].file.as_deref(), Some(std::path::Path::new("p.jpg")));
        assert!(history_entry(&queue.items[1], std::path::Path::new("p.jpg"), String::new()).is_none());
    }

    #[test]
    fn cancel_retry_and_remove_follow_the_item_state() {
        let mut queue = Queue::default();
        let (running, waiting) = (queue.add(download("a")).unwrap(), queue.add(download("b")).unwrap());
        queue.start_next(Limits { download: 1, video: 1, image: 1 });
        queue.cancel(waiting);
        assert_eq!(queue.items[1].state, State::Cancelled, "a waiting item stops at once");
        queue.cancel(running);
        assert_eq!(queue.items[0].state, State::Running, "a running item keeps its slot until its worker stops");
        queue.remove(running);
        assert_eq!(queue.items.len(), 2, "a running item cannot be removed");
        assert!(queue.start_next(Limits { download: 1, video: 1, image: 1 }).is_empty());
        finish(&mut queue, running, Outcome::Cancelled);
        // Retry keeps the position but takes a new id, so the old run's messages no longer match.
        queue.retry(running);
        let retried = queue.items[0].id;
        assert!(retried != running && queue.items[0].state == State::Waiting);
        assert!(matches!(queue.apply(Msg { id: running, update: Update::Finished(Outcome::Done) }), Applied::Nothing));
        queue.retry(retried);
        assert_eq!(queue.items[0].id, retried, "only failed or cancelled items are retried");
        queue.remove(waiting);
        queue.clear_finished();
        assert_eq!(queue.items.len(), 1);
        queue.cancel_all();
        assert!(!queue.active());
        queue.clear_finished();
        assert!(queue.items.is_empty());
    }

    #[test]
    fn adding_many_skips_what_is_already_queued_and_repeats_in_the_batch() {
        let mut queue = Queue::default();
        queue.add(image("a.png"));
        let added = queue.add_all(["a.png", "b.png", "b.png", "c.png"].map(image).into_iter().chain([download("x"), download("x")]));
        assert_eq!((added, queue.items.len()), (3, 4));
        // The same picture to another format is other work.
        assert_eq!(queue.add_all([Work::Convert { source: "a.png".into(), target: Target::Image(ImageTarget::Webp) }]), 1);
    }

    #[test]
    fn video_and_audio_streams_fill_the_bar_one_after_the_other() {
        assert_eq!(download_progress("137+140", "137", 50.0), Some(0.25));
        assert_eq!(download_progress("137+140", "137", 100.0), Some(0.5), "the video done is half of the download");
        assert_eq!(download_progress("137+140", "140", 50.0), Some(0.75));
        assert_eq!(download_progress("140", "140", 40.0), Some(0.4), "a single stream (audio only)");
        assert_eq!(download_progress("137+140", "137+140", 40.0), Some(0.4), "both streams fetched together by ffmpeg");
        assert_eq!(download_progress("", "", 40.0), Some(0.4), "a site that names no streams");
        assert_eq!(download_progress("137+140", "", 100.0), None, "a subtitle does not move the bar");
    }

    #[test]
    fn a_cancelled_job_reports_cancelled_without_starting_anything() {
        let job = Job::default();
        job.cancel();
        let updates = std::cell::RefCell::new(Vec::new());
        run(image("no-such-file.png"), &job, false, &|update| updates.borrow_mut().push(update));
        assert_eq!(updates.borrow().last(), Some(&Update::Finished(Outcome::Cancelled)));
    }
}
