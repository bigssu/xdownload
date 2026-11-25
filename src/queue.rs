//! The work queue: downloads and conversions waiting, running and finished.
//!
//! The queue itself is plain data owned by the UI thread. Every running item has its own
//! thread and `Job`; the thread reports through a channel of `Msg`. A retry gets a new id, so
//! a late message from the cancelled run can never be mistaken for the new one.
use crate::converter::{self, Encoder, Target};
use crate::downloader::{self, Event, Options};
use crate::proc::{Job, CANCELLED};
use crate::{binaries, settings};
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
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
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
    /// A file was completed.
    File(PathBuf),
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
    /// A download finished this file.
    Downloaded(&'a Item, PathBuf),
    /// An item reached Done, Failed or Cancelled.
    Finished,
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
        let same = |other: &Work| match (other, &work) {
            (Work::Download { url: a, .. }, Work::Download { url: b, .. }) => a == b,
            _ => *other == work,
        };
        if self.items.iter().any(|item| !item.finished() && same(&item.work)) {
            return None;
        }
        self.next_id += 1;
        self.items.push(Item { id: self.next_id, title: default_title(&work), work, state: State::Waiting, progress: 0.0, note: Note::None, file: None, job: None });
        Some(self.next_id)
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
            Update::File(path) => {
                item.file = Some(path.clone());
                if matches!(item.work, Work::Download { .. }) {
                    return Applied::Downloaded(item, path);
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

/// `name.f137.mp4` (an intermediate stream file) → `name`; `name.mp4` → `name`.
pub fn display_title(file_name: &str) -> String {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(stem, _)| stem);
    match stem.rsplit_once(".f") {
        Some((title, id)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && id.bytes().next().is_some_and(|b| b.is_ascii_digit()) => title.to_owned(),
        _ => stem.to_owned(),
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
            downloader::download_with_auto_update(url, options, &ytdlp, &ffmpeg, job, &mut || send(Update::Note(Note::Retrying)), &mut |event| match event {
                Event::Progress { percent, speed, eta } => {
                    send(Update::Progress(percent / 100.0));
                    send(Update::Note(Note::Speed { speed: speed.to_owned(), eta: eta.to_owned() }));
                }
                Event::Item { index, total } => playlist = Some((index, total)),
                Event::Destination(name) => {
                    let title = display_title(name);
                    send(Update::Title(match playlist {
                        Some((index, total)) => format!("({index}/{total}) {title}"),
                        None => title,
                    }));
                }
                Event::File(path) => send(Update::File(PathBuf::from(path))),
            }).map_err(|error| error.to_string())
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
                send(Update::File(converted.dest));
            }
            Ok(converted.code)
        }
    }
}

/// History entry for a file a download finished.
pub fn history_entry(item: &Item, file: &std::path::Path) -> Option<settings::Entry> {
    let Work::Download { url, options } = &item.work else { return None };
    Some(settings::Entry {
        url: url.clone(),
        title: file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default(),
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
        let Applied::Downloaded(item, file) = queue.apply(Msg { id, update: Update::File("C:/out/Song.mp4".into()) }) else { panic!("a finished download file is reported") };
        let entry = history_entry(item, &file).unwrap();
        assert_eq!((entry.title.as_str(), entry.format.as_str(), entry.quality.as_str(), entry.path.as_str(), entry.url.as_str()), ("Song", "mp4", "720p", "C:/out", "a"));
        assert!(entry.file.ends_with("Song.mp4"));
        finish(&mut queue, id, Outcome::Failed("exit 1".into()));
        assert_eq!((queue.items[0].state, &queue.items[0].note), (State::Failed, &Note::Error("exit 1".into())));
        assert!(matches!(queue.apply(Msg { id, update: Update::Progress(0.1) }), Applied::Nothing), "late messages are dropped");
        assert!(!queue.active());
        // A conversion's file is kept on the item but is not a download for the history.
        let id = queue.add(image("p.png")).unwrap();
        queue.start_next(LIMITS);
        assert!(matches!(queue.apply(Msg { id, update: Update::File("p.jpg".into()) }), Applied::Nothing));
        assert_eq!(queue.items[1].file.as_deref(), Some(std::path::Path::new("p.jpg")));
        assert!(history_entry(&queue.items[1], std::path::Path::new("p.jpg")).is_none());
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
    fn stream_suffixes_are_dropped_from_titles() {
        assert_eq!(display_title("Me at the zoo.f133.mp4"), "Me at the zoo");
        assert_eq!(display_title("Me at the zoo.f140-drc.m4a"), "Me at the zoo");
        assert_eq!(display_title("노래 제목.mp4"), "노래 제목");
        assert_eq!(display_title("v1.final.mp4"), "v1.final");
        assert_eq!(display_title("noext"), "noext");
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
