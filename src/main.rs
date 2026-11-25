#![windows_subsystem = "windows"]
//! Xdownload2 — YouTube downloader and local media converter.
//!
//! The UI thread only draws and schedules. Every queued download or conversion runs on its
//! own worker thread (see `queue`), reports through a channel and asks for a repaint.

mod binaries;
mod converter;
mod crashlog;
mod downloader;
mod i18n;
mod proc;
mod queue;
mod settings;
mod tabs;
mod ui;
mod update;

use eframe::egui::{self, pos2, vec2, Color32, Key, Rect, RichText, Sense, Stroke, StrokeKind, ViewportCommand};
use i18n::tr;
use proc::Job;
use queue::{Applied, Limits, Msg, Queue, Work};
use settings::{Entry, Settings};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use ui::theme::{self, ACCENT, BG, LINE, MUTED, RED, SUCCESS};
use ui::widgets::{self as w, Tip};

const APP_NAME: &str = "Xdownload2";
const TITLE_H: f32 = 50.0;
const FOOTER_H: f32 = 32.0;
/// Width of the invisible resize grab zone along the window edges; corners get `CORNER`.
const GRIP: f32 = 8.0;
const CORNER: f32 = 24.0;
const WINDOW: [f32; 2] = [660.0, 860.0];
const MIN_WINDOW: [f32; 2] = [600.0, 640.0];

/// A status line kept as key + values, so it follows a language switch.
#[derive(Clone, PartialEq, Debug)]
struct Status {
    key: &'static str,
    values: Vec<(&'static str, String)>,
}

impl Status {
    fn new(key: &'static str) -> Self {
        Self { key, values: Vec::new() }
    }

    fn with(mut self, name: &'static str, value: impl ToString) -> Self {
        self.values.push((name, value.to_string()));
        self
    }

    fn text(&self) -> String {
        let values: Vec<(&str, &str)> = self.values.iter().map(|(name, value)| (*name, value.as_str())).collect();
        i18n::fill(self.key, &values)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tone {
    Normal,
    Good,
    Bad,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tab {
    Download,
    Convert,
    History,
    Settings,
}

const TABS: [Tab; 4] = [Tab::Download, Tab::Convert, Tab::History, Tab::Settings];

/// Reports from the start-up thread that readies the tools.
enum Background {
    Status(Status, Tone),
    /// yt-dlp was updated today.
    Updated,
    Encoder(converter::Encoder),
    Release(update::Release),
}

/// Non-empty trimmed lines of the link box.
fn parse_urls(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Opens a file, folder or web link with whatever Windows uses for it.
fn open(target: &str) {
    let _ = proc::command("explorer").arg(target).spawn();
}

/// Opens the folder holding `file` with the file selected.
fn reveal(file: &Path) {
    let _ = proc::command("explorer").arg(format!("/select,{}", file.display())).spawn();
}

/// Readies yt-dlp / ffmpeg ahead of the first download, keeps yt-dlp current (once a day),
/// finds the hardware encoder and looks for a newer app version.
fn startup(update_ytdlp: bool, use_gpu: bool, tx: Sender<Background>, ctx: egui::Context) {
    let send = |message| {
        let _ = tx.send(message);
        ctx.request_repaint();
    };
    let job = Job::default();
    let mut progress = |fraction: f32| send(Background::Status(Status::new("prep_tools").with("p", (fraction * 100.0) as u32), Tone::Normal));
    let tools = binaries::ensure_ytdlp(&job, &mut progress).and_then(|ytdlp| Ok((ytdlp, binaries::ensure_ffmpeg(&job, &mut progress)?)));
    match tools {
        Ok((ytdlp, ffmpeg)) => {
            if update_ytdlp {
                send(Background::Status(Status::new("prep_update"), Tone::Normal));
                downloader::update_once(&ytdlp, &job);
                send(Background::Updated);
            }
            if use_gpu {
                send(Background::Encoder(converter::detect_encoder(&ffmpeg)));
            }
            send(Background::Status(Status::new("ready"), Tone::Normal));
        }
        // Not fatal: the first download or conversion tries again and reports its own error.
        Err(error) => send(Background::Status(Status::new("prep_fail").with("error", error), Tone::Bad)),
    }
    if let Some(release) = update::check() {
        send(Background::Release(release));
    }
}

struct App {
    s: Settings,
    /// What is on disk, so a change is written once.
    stored: Settings,
    tab: Tab,
    urls: String,
    history: Vec<Entry>,
    queue: Queue,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    background: Receiver<Background>,
    /// Footer line: the latest notice or start-up status.
    notice: (Status, Tone),
    /// The hardware encoder found at start-up; `None` until known.
    encoder: Option<converter::Encoder>,
    /// A newer app version on the portal.
    release: Option<update::Release>,
    focused: bool,
    /// Clipboard text already offered, so the same copy is pasted once.
    last_clipboard: String,
    /// The queue had work last frame.
    active: bool,
    /// A download finished since the queue last went idle.
    downloaded: bool,
    /// `--screenshot <png>`: draw demo content, save one frame and quit.
    shot: Option<PathBuf>,
    frame_no: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, s: Settings, shot: Option<PathBuf>, tab: Tab, inputs: Vec<String>) -> Self {
        theme::setup(&cc.egui_ctx);
        let (background_tx, background) = mpsc::channel();
        let mut app = Self::fresh(s, shot, background);
        app.tab = tab;
        if app.shot.is_none() {
            let (update_ytdlp, use_gpu, ctx) = (app.s.last_update_day != today(), app.s.gpu, cc.egui_ctx.clone());
            std::thread::spawn(move || startup(update_ytdlp, use_gpu, background_tx, ctx));
            app.take_inputs(inputs);
        }
        app
    }

    fn fresh(s: Settings, shot: Option<PathBuf>, background: Receiver<Background>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            stored: s.clone(), s, tab: Tab::Download, urls: String::new(), history: Vec::new(), queue: Queue::default(), tx, rx, background,
            notice: (Status::new("ready"), Tone::Normal), encoder: None, release: None, focused: false, last_clipboard: String::new(),
            active: false, downloaded: false, shot, frame_no: 0,
        };
        if app.shot.is_some() {
            app.demo();
        } else {
            app.history = settings::load_history();
        }
        app
    }

    /// Sample content for `--screenshot`, so the picture shows a working state.
    fn demo(&mut self) {
        self.urls = "https://www.youtube.com/watch?v=dQw4w9WgXcQ\nhttps://youtu.be/9bZkp7q19f0".into();
        self.s.download_dir = r"C:\Users\me\Downloads".into();
        self.queue = Queue::demo(self.options());
        self.encoder = Some(converter::Encoder::Nvenc);
        self.release = Some(update::Release { version: "2.1.0".into(), url: String::new() });
        self.history = (1..=4).map(|n| Entry {
            url: format!("https://youtu.be/sample{n}"),
            title: format!("Sample video title {n}"),
            file: String::new(),
            format: ["mp4", "mp3", "m4a", "mp4"][n - 1].into(),
            quality: ["1080p", "best", "best", "2160p"][n - 1].into(),
            path: self.s.download_dir.clone(),
            when: format!("2026-11-2{n} 1{n}:30"),
        }).collect();
    }

    /// Links and files given on the command line (or dropped on the exe).
    fn take_inputs(&mut self, inputs: Vec<String>) {
        let (links, paths): (Vec<String>, Vec<String>) = inputs.into_iter().partition(|input| input.starts_with("http://") || input.starts_with("https://"));
        if !links.is_empty() {
            self.urls = links.join("\n");
        }
        self.add_dropped(paths.into_iter().map(PathBuf::from).collect());
    }

    fn set_notice(&mut self, status: Status, tone: Tone) {
        self.notice = (status, tone);
    }

    fn format(&self) -> downloader::Format {
        downloader::Format::ALL[(self.s.format as usize).min(2)]
    }

    /// The download options as currently chosen.
    fn options(&self) -> downloader::Options {
        downloader::Options {
            format: self.format(), height: self.s.height, subs: self.s.subs, embed_info: self.s.embed_info,
            playlist: self.s.playlist, out_dir: self.s.download_dir.clone(),
        }
    }

    fn video_target(&self) -> converter::Target {
        use converter::VideoTarget::*;
        converter::Target::Video { target: [Mp4, Mp3, Gif][(self.s.video_target as usize).min(2)], height: self.s.video_height, quality: self.s.video_quality }
    }

    fn image_target(&self) -> converter::Target {
        use converter::ImageTarget::*;
        converter::Target::Image([Jpg, Png, Webp][(self.s.image_target as usize).min(2)])
    }

    /// Queues every link in the box and empties it.
    fn add_downloads(&mut self) {
        let urls = parse_urls(&self.urls);
        if urls.is_empty() {
            return self.set_notice(Status::new("need_url"), Tone::Bad);
        }
        let options = self.options();
        let added = urls.into_iter().filter(|url| self.queue.add(Work::Download { url: url.clone(), options: options.clone() }).is_some()).count();
        self.urls.clear();
        self.report_added(added, "already");
    }

    fn report_added(&mut self, added: usize, none_key: &'static str) {
        if added > 0 {
            self.set_notice(Status::new("added").with("n", added), Tone::Good);
        } else {
            self.set_notice(Status::new(none_key), Tone::Bad);
        }
    }

    /// Queues the media under `sources` for `target`; returns how many were added.
    fn add_conversions(&mut self, sources: &[PathBuf], target: converter::Target) -> usize {
        let files = converter::collect_media_files(sources, target, self.s.subfolders);
        files.into_iter().filter(|source| self.queue.add(Work::Convert { source: source.clone(), target }).is_some()).count()
    }

    /// Dropped files go to the converter that fits them; folders are searched for both kinds.
    fn add_dropped(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let (folders, files): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|path| path.is_dir());
        let (mut videos, mut images) = (folders.clone(), folders);
        for file in files {
            match converter::is_video_file(&file) {
                Some(true) => videos.push(file),
                Some(false) => images.push(file),
                None => {}
            }
        }
        let added = self.add_conversions(&videos, self.video_target()) + self.add_conversions(&images, self.image_target());
        self.tab = Tab::Convert;
        self.report_added(added, "need_convert_files");
    }

    fn pick_sources(&self, frame: &eframe::Frame, target: converter::Target, folder: bool) -> Vec<PathBuf> {
        let video = target.is_video();
        let dialog = rfd::FileDialog::new().set_parent(frame);
        if folder {
            return dialog.set_title(tr(if video { "pick_video_folder" } else { "pick_image_folder" })).pick_folder().into_iter().collect();
        }
        dialog.set_title(tr(if video { "pick_video_files" } else { "pick_image_files" }))
            .add_filter(tr(if video { "video_files" } else { "image_files" }), target.sources())
            .add_filter(tr("all_files"), &["*"])
            .pick_files().unwrap_or_default()
    }

    /// Adds copied YouTube links when the window comes to the front.
    fn paste_on_focus(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused);
        let gained = focused && !self.focused;
        self.focused = focused;
        if !gained || !self.s.auto_paste || self.shot.is_some() {
            return;
        }
        let Ok(text) = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) else { return };
        if text == self.last_clipboard {
            return;
        }
        let links = update::new_links(&text, &self.urls);
        self.last_clipboard = text;
        if links.is_empty() {
            return;
        }
        if !self.urls.is_empty() && !self.urls.ends_with('\n') {
            self.urls.push('\n');
        }
        self.urls.push_str(&links.join("\n"));
        self.set_notice(Status::new("pasted").with("n", links.len()), Tone::Good);
    }

    /// Applies everything the workers and the start-up thread reported since the last frame.
    fn drain(&mut self) {
        while let Ok(message) = self.background.try_recv() {
            match message {
                Background::Status(status, tone) => self.set_notice(status, tone),
                Background::Updated => self.s.last_update_day = today(),
                Background::Encoder(encoder) => self.encoder = Some(encoder),
                Background::Release(release) => self.release = Some(release),
            }
        }
        while let Ok(msg) = self.rx.try_recv() {
            if let Applied::Downloaded(item, file) = self.queue.apply(msg) {
                let entry = queue::history_entry(item, &file);
                self.downloaded = true;
                if let (Some(entry), None) = (entry, &self.shot) {
                    // Another running window may have added entries since this one read the file.
                    self.history = settings::load_history();
                    settings::push_history(&mut self.history, entry);
                    settings::store_history(&self.history);
                }
            }
        }
    }

    /// Starts waiting items within the limits and notices when the queue runs dry.
    fn schedule(&mut self, ctx: &egui::Context) {
        // Two videos at once only when a hardware encoder is known to work; until then (not
        // probed yet, or none found) conversions may land on the CPU, one at a time.
        let use_gpu = self.s.gpu && self.encoder.is_some_and(|encoder| encoder != converter::Encoder::Cpu);
        for (id, work, job) in self.queue.start_next(Limits::new(self.s.parallel(), use_gpu)) {
            let (tx, ctx, use_gpu) = (self.tx.clone(), ctx.clone(), self.s.gpu);
            std::thread::spawn(move || {
                queue::run(work, &job, use_gpu, &|update| {
                    let _ = tx.send(Msg { id, update });
                    ctx.request_repaint();
                });
            });
        }
        let active = self.queue.active();
        if self.active && !active {
            if self.downloaded && self.s.open_when_done {
                open(&self.s.download_dir);
            }
            self.downloaded = false;
        }
        self.active = active;
    }

    fn title_bar(&mut self, ctx: &egui::Context) {
        let corners = egui::CornerRadius { nw: theme::WINDOW_R, ne: theme::WINDOW_R, sw: 0, se: 0 };
        egui::TopBottomPanel::top("title").exact_height(TITLE_H)
            .frame(egui::Frame::default().fill(BG).corner_radius(corners).inner_margin(egui::Margin::symmetric(12, 7)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (logo, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    w::paint_gradient(ui.painter(), logo, 8.0, ACCENT, theme::ACCENT_END);
                    ui::icons::paint(ui.painter(), logo.shrink(5.0), "logo", Color32::WHITE);
                    if ui.available_width() >= 720.0 {
                        ui.label(RichText::new(APP_NAME).strong().size(14.0));
                    }
                    let mut index = TABS.iter().position(|tab| *tab == self.tab).unwrap_or(0);
                    let tabs = [("download", tr("tab_download")), ("convert", tr("tab_convert")), ("history", tr("tab_history")), ("settings", tr("tab_settings"))];
                    if w::segmented_tips(ui, "tabs", &tabs, &mut index, 208.0).changed() {
                        self.tab = TABS[index];
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::icon_button(ui, "close", tr("close")).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        let toggle_max = |ctx: &egui::Context| {
                            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
                        };
                        if w::icon_button(ui, "max", tr("maximize")).clicked() {
                            toggle_max(ctx);
                        }
                        if w::icon_button(ui, "min", tr("minimize")).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        ui.separator();
                        let mut language = usize::from(self.s.language == 1);
                        if w::segmented(ui, "language", &["한", "En"], &mut language, 76.0).tip(tr("lang_tip")).changed() {
                            self.s.language = language as u8;
                            i18n::set_language(self.s.language);
                        }
                        if let Some(release) = &self.release {
                            if w::icon_button(ui, "update", &i18n::fill("update_available", &[("v", &release.version)])).clicked() {
                                open(&release.url);
                            }
                        }
                        // The rest of the bar moves the window.
                        let (_, drag) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                        if drag.drag_started() {
                            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                        }
                        if drag.double_clicked() {
                            toggle_max(ctx);
                        }
                    });
                });
            });
    }

    fn footer(&self, ctx: &egui::Context) {
        let corners = egui::CornerRadius { nw: 0, ne: 0, sw: theme::WINDOW_R, se: theme::WINDOW_R };
        egui::TopBottomPanel::bottom("footer").exact_height(FOOTER_H)
            .frame(egui::Frame::default().fill(BG).corner_radius(corners).inner_margin(egui::Margin::symmetric(18, 0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (status, tone) = &self.notice;
                    let color = match tone {
                        Tone::Normal => MUTED,
                        Tone::Good => SUCCESS,
                        Tone::Bad => RED,
                    };
                    ui.add(egui::Label::new(RichText::new(status.text()).size(12.0).color(color)).truncate());
                });
            });
    }

    /// Borderless windows have no system frame, so the edges are made draggable here.
    fn resize_grips(&self, ctx: &egui::Context) {
        use egui::viewport::ResizeDirection as D;
        let r = ctx.screen_rect();
        // Later zones win, so corners come last.
        let zones = [
            (Rect::from_min_max(r.min, pos2(r.min.x + GRIP, r.max.y)), D::West, egui::CursorIcon::ResizeHorizontal),
            (Rect::from_min_max(pos2(r.max.x - GRIP, r.min.y), r.max), D::East, egui::CursorIcon::ResizeHorizontal),
            (Rect::from_min_max(pos2(r.min.x, r.max.y - GRIP), r.max), D::South, egui::CursorIcon::ResizeVertical),
            (Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 3.0)), D::North, egui::CursorIcon::ResizeVertical),
            (Rect::from_min_max(pos2(r.min.x, r.max.y - CORNER), pos2(r.min.x + CORNER, r.max.y)), D::SouthWest, egui::CursorIcon::ResizeNeSw),
            (Rect::from_min_max(r.max - vec2(CORNER, CORNER), r.max), D::SouthEast, egui::CursorIcon::ResizeNwSe),
        ];
        egui::Area::new(egui::Id::new("grips")).fixed_pos(r.min).order(egui::Order::Foreground).interactable(true).show(ctx, |ui| {
            for (k, (zone, direction, icon)) in zones.into_iter().enumerate() {
                let response = ui.interact(zone, egui::Id::new(("grip", k)), Sense::drag());
                if response.hovered() {
                    ctx.set_cursor_icon(icon);
                }
                if response.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
            }
        });
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("edge")))
            .rect_stroke(r, theme::WINDOW_R, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else { return };
        self.frame_no += 1;
        ctx.request_repaint();
        // A few frames let fonts load and animations settle.
        if self.frame_no == 30 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| i.events.iter().find_map(|event| match event {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        }));
        if let Some(shot) = shot {
            let [width, height] = shot.size;
            let bytes: Vec<u8> = shot.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buffer) = image::RgbaImage::from_raw(width as u32, height as u32, bytes) {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = buffer.save(&path);
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

/// "v2.0.0 · 11-25 10:12": package version plus the local time `build.rs` stamped.
fn build_version() -> String {
    let secs: i64 = env!("XDOWNLOAD2_BUILD_TIME").parse().unwrap_or(0);
    let time = chrono::DateTime::from_timestamp(secs, 0).map(|t| t.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string());
    format!("v{} · {}", env!("CARGO_PKG_VERSION"), time.unwrap_or_default())
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.drain();
        // Closing the window must not leave yt-dlp / ffmpeg running.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.queue.cancel_all_blocking();
            downloader::UPDATE_JOB.cancel();
        }
        self.paste_on_focus(ctx);
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|file| file.path.clone()).collect());
        self.add_dropped(dropped);
        let (escape, enter) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        if escape && self.queue.active() {
            self.queue.cancel_all();
        }
        // Inside the link box Enter is a line break; anywhere else it starts the download.
        if enter && self.tab == Tab::Download && ctx.memory(|m| m.focused().is_none()) {
            self.add_downloads();
        }
        self.title_bar(ctx);
        self.footer(ctx);
        self.content(ctx, frame);
        self.resize_grips(ctx);
        if self.shot.is_none() {
            self.schedule(ctx);
            if self.s != self.stored {
                self.s.store();
                self.stored = self.s.clone();
            }
        }
        self.screenshot_step(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // the rounded window corners stay see-through
    }
}

fn main() -> eframe::Result<()> {
    crashlog::install();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let shot = value("--screenshot").map(PathBuf::from);
    // Screenshots start from defaults so they never depend on, or touch, the user's settings.
    let mut s = if shot.is_some() { Settings::default() } else { Settings::load() };
    let mut tab = Tab::Download;
    if shot.is_some() {
        s.language = u8::from(value("--lang").as_deref() == Some("en"));
        tab = match value("--tab").as_deref() {
            Some("convert") => Tab::Convert,
            Some("history") => Tab::History,
            Some("settings") => Tab::Settings,
            _ => Tab::Download,
        };
    }
    // Everything that is not an option is a link or a file to work on.
    let inputs: Vec<String> = if shot.is_some() { Vec::new() } else { args.iter().filter(|a| !a.starts_with("--")).cloned().collect() };
    // The first frame must already be in the saved language.
    i18n::set_language(s.language);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size(WINDOW)
        .with_min_inner_size(MIN_WINDOW)
        .with_transparent(true)
        .with_decorations(false)
        .with_drag_and_drop(true)
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/xdownload2.png")).expect("bundled icon"));
    if shot.is_some() {
        viewport = viewport.with_active(false);
    }
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(App::new(cc, s, shot, tab, inputs)))))
}

#[cfg(test)]
mod tests;
