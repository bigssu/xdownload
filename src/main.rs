#![windows_subsystem = "windows"]
//! Xdownload2 — YouTube MP4/MP3 downloader and local media converter.
//!
//! The UI thread only draws; downloads and conversions run on one worker thread that reports
//! back through a channel and asks for a repaint.

mod binaries;
mod converter;
mod crashlog;
mod downloader;
mod i18n;
mod proc;
mod settings;
mod ui;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Rect, RichText, Sense, Stroke, StrokeKind, ViewportCommand};
use i18n::tr;
use proc::{Job, CANCELLED};
use settings::{Entry, Settings};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use ui::theme::{self, ACCENT, BG, LINE, MUTED, PANEL, RAISED, RED, SUCCESS, TEXT, WELL};
use ui::widgets::{self as w, Kind, Tip};

const APP_NAME: &str = "Xdownload2";
const TITLE_H: f32 = 50.0;
/// Width of the invisible resize grab zone along the window edges; corners get `CORNER`.
const GRIP: f32 = 8.0;
const CORNER: f32 = 24.0;
const WINDOW: [f32; 2] = [560.0, 760.0];
const MIN_WINDOW: [f32; 2] = [440.0, 600.0];

/// A status line kept as key + values, so it follows a language switch while work runs.
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

enum Msg {
    /// 0..=1
    Progress(f32),
    Status(Status),
    Recorded(Entry),
    Done(Status, Tone),
}

/// The worker's side of the channel: every message also wakes the UI.
#[derive(Clone)]
struct Link {
    tx: Sender<Msg>,
    ctx: egui::Context,
}

impl Link {
    fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
        self.ctx.request_repaint();
    }

    fn progress(&self, fraction: f32) {
        self.send(Msg::Progress(fraction));
    }

    fn status(&self, status: Status) {
        self.send(Msg::Status(status));
    }
}

/// Final line of a batch: cancelled / some failed / all fine.
fn summary(done_key: &'static str, cancelled: bool, success: usize, failed: usize) -> (Status, Tone) {
    if cancelled {
        (Status::new("cancelled").with("s", success).with("f", failed), if failed > 0 { Tone::Bad } else { Tone::Normal })
    } else if failed > 0 {
        (Status::new("done_fail").with("s", success).with("f", failed), Tone::Bad)
    } else {
        (Status::new(done_key).with("s", success), Tone::Good)
    }
}

/// Non-empty trimmed lines of the link box.
fn parse_urls(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect()
}

fn history_label(entry: &Entry) -> String {
    format!("[{}] {} · {}", entry.when, entry.format.to_uppercase(), entry.url)
}

/// A snapshot of the options taken when the download starts.
struct DownloadPlan {
    urls: Vec<String>,
    mp3: bool,
    quality: &'static str,
    out_dir: String,
}

fn download_worker(plan: DownloadPlan, job: Arc<Job>, link: Link) {
    link.status(Status::new("prep_binaries"));
    let binaries = binaries::ensure_ytdlp(&job, &mut |f| link.progress(f)).and_then(|ytdlp| Ok((ytdlp, binaries::ensure_ffmpeg(&job, &mut |f| link.progress(f))?)));
    let (ytdlp, ffmpeg) = match binaries {
        Ok(paths) => paths,
        Err(_) if job.cancelled() => return link.send(done(summary("done_ok", true, 0, 0))),
        Err(error) => return link.send(Msg::Done(Status::new("binary_fail").with("error", error), Tone::Bad)),
    };
    let total = plan.urls.len();
    let (mut success, mut failed) = (0, 0);
    for (index, url) in plan.urls.iter().enumerate() {
        if job.cancelled() {
            break;
        }
        let numbered = |key| Status::new(key).with("i", index + 1).with("n", total);
        link.progress(0.0);
        link.status(numbered("downloading"));
        let request = downloader::Request { url, mp3: plan.mp3, quality: plan.quality, out_dir: &plan.out_dir, ytdlp: &ytdlp, ffmpeg: &ffmpeg };
        let result = downloader::download_with_auto_update(&request, &job, &mut || link.status(Status::new("retry_update")), &mut |line| {
            if let Some(percent) = downloader::percent(line) {
                link.progress(percent / 100.0);
            }
            if let Some(name) = downloader::destination(line) {
                link.status(numbered("progress_file").with("name", name));
            }
        });
        match result {
            Ok(0) => {
                success += 1;
                link.send(Msg::Recorded(Entry {
                    url: url.clone(),
                    format: if plan.mp3 { "mp3" } else { "mp4" }.into(),
                    quality: plan.quality.into(),
                    path: plan.out_dir.clone(),
                    when: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                }));
            }
            Ok(CANCELLED) => break,
            Ok(_) => failed += 1,
            Err(error) => {
                failed += 1;
                link.status(numbered("item_error").with("error", error));
            }
        }
    }
    // Individual failures are overwritten by the next item's status, so the totals come last.
    link.send(done(summary("done_ok", job.cancelled(), success, failed)));
}

fn convert_worker(kind: converter::Kind, files: Vec<PathBuf>, job: Arc<Job>, link: Link) {
    link.status(Status::new("prep_ffmpeg"));
    let ffmpeg = match binaries::ensure_ffmpeg(&job, &mut |f| link.progress(f)) {
        Ok(path) => path,
        Err(_) if job.cancelled() => return link.send(done(summary("convert_done_ok", true, 0, 0))),
        Err(error) => return link.send(Msg::Done(Status::new("binary_fail").with("error", error), Tone::Bad)),
    };
    let total = files.len();
    let (mut success, mut failed) = (0, 0);
    for (index, path) in files.iter().enumerate() {
        if job.cancelled() {
            break;
        }
        let numbered = |key| Status::new(key).with("i", index + 1).with("n", total);
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        link.progress(0.0);
        link.status(numbered("convert_progress_file").with("name", name));
        let result = match kind {
            converter::Kind::Video => converter::convert_video(path, &ffmpeg, &job, &mut |percent| link.progress(percent as f32 / 100.0)),
            converter::Kind::Image => converter::convert_image(path, &ffmpeg, &job),
        };
        match result {
            Ok(0) => {
                success += 1;
                link.progress(1.0);
            }
            Ok(CANCELLED) => break,
            Ok(_) => failed += 1,
            Err(error) => {
                failed += 1;
                link.status(numbered("convert_item_error").with("error", error));
            }
        }
    }
    link.send(done(summary("convert_done_ok", job.cancelled(), success, failed)));
}

fn done((status, tone): (Status, Tone)) -> Msg {
    Msg::Done(status, tone)
}

struct App {
    s: Settings,
    /// What is on disk, so a change is written once.
    stored: Settings,
    urls: String,
    history: Vec<Entry>,
    /// The running batch; `None` when idle.
    job: Option<Arc<Job>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    progress: f32,
    status: Status,
    tone: Tone,
    /// `--screenshot <png>`: draw demo content, save one frame and quit.
    shot: Option<PathBuf>,
    frame_no: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, s: Settings, shot: Option<PathBuf>) -> Self {
        theme::setup(&cc.egui_ctx);
        Self::fresh(s, shot)
    }

    fn fresh(s: Settings, shot: Option<PathBuf>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            stored: s.clone(), s, urls: String::new(), history: Vec::new(), job: None, tx, rx,
            progress: 0.0, status: Status::new("status_idle"), tone: Tone::Normal, shot, frame_no: 0,
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
        self.progress = 0.42;
        self.status = Status::new("progress_file").with("i", 1).with("n", 2).with("name", "Sample video title.mp4");
        self.history = (1..=3).map(|n| Entry {
            url: format!("https://youtu.be/sample{n}"),
            format: if n == 2 { "mp3" } else { "mp4" }.into(),
            quality: "best".into(),
            path: self.s.download_dir.clone(),
            when: format!("2026-11-2{n} 1{n}:30"),
        }).collect();
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn set_status(&mut self, status: Status, tone: Tone) {
        self.status = status;
        self.tone = tone;
    }

    /// Applies everything the worker reported since the last frame.
    fn drain(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Progress(fraction) => self.progress = fraction,
                // "Cancelling..." stays up until the worker really stops.
                Msg::Status(status) => {
                    if !self.job.as_ref().is_some_and(|job| job.cancelled()) {
                        self.set_status(status, Tone::Normal);
                    }
                }
                Msg::Recorded(entry) => {
                    settings::push_history(&mut self.history, entry);
                    settings::store_history(&self.history);
                }
                Msg::Done(status, tone) => {
                    self.job = None;
                    self.progress = 0.0;
                    self.set_status(status, tone);
                }
            }
        }
    }

    fn launch(&mut self, ctx: &egui::Context, work: impl FnOnce(Arc<Job>, Link) + Send + 'static) {
        let job = Arc::new(Job::default());
        self.job = Some(job.clone());
        self.progress = 0.0;
        self.set_status(Status::new("status_idle"), Tone::Normal);
        let link = Link { tx: self.tx.clone(), ctx: ctx.clone() };
        std::thread::spawn(move || work(job, link));
    }

    fn cancel(&mut self) {
        if let Some(job) = self.job.clone() {
            self.set_status(Status::new("cancelling"), Tone::Normal);
            // Killing the process tree can take a moment; keep it off the UI thread.
            std::thread::spawn(move || job.cancel());
        }
    }

    fn start_download(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        let urls = parse_urls(&self.urls);
        if urls.is_empty() {
            return self.set_status(Status::new("need_url"), Tone::Bad);
        }
        let plan = DownloadPlan { urls, mp3: self.s.mp3, quality: self.s.quality_name(), out_dir: self.s.download_dir.clone() };
        self.launch(ctx, move |job, link| download_worker(plan, job, link));
    }

    fn start_conversion(&mut self, ctx: &egui::Context, kind: converter::Kind, sources: Vec<PathBuf>) {
        if self.busy() || sources.is_empty() {
            return;
        }
        let files = converter::collect_media_files(&sources, kind);
        if files.is_empty() {
            return self.set_status(Status::new("need_convert_files"), Tone::Bad);
        }
        self.launch(ctx, move |job, link| convert_worker(kind, files, job, link));
    }

    fn pick_sources(&self, frame: &eframe::Frame, kind: converter::Kind, folder: bool) -> Vec<PathBuf> {
        let video = kind == converter::Kind::Video;
        let dialog = rfd::FileDialog::new().set_parent(frame);
        if folder {
            return dialog.set_title(tr(if video { "pick_video_folder" } else { "pick_image_folder" })).pick_folder().into_iter().collect();
        }
        dialog.set_title(tr(if video { "pick_video_files" } else { "pick_image_files" }))
            .add_filter(tr(if video { "video_files" } else { "image_files" }), kind.extensions())
            .add_filter(tr("all_files"), &["*"])
            .pick_files().unwrap_or_default()
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
                    ui.label(RichText::new(APP_NAME).strong().size(14.0));
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
                        ui.add_space(6.0);
                        ui.label(RichText::new(build_version()).size(11.0).color(MUTED));
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

    fn content(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let corners = egui::CornerRadius { nw: 0, ne: 0, sw: theme::WINDOW_R, se: theme::WINDOW_R };
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(PANEL).corner_radius(corners).inner_margin(egui::Margin::same(18)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                let busy = self.busy();
                let half = (ui.available_width() - 8.0) / 2.0;

                section(ui, tr("url_label"));
                egui::ScrollArea::vertical().id_salt("urls").max_height(92.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.urls).desired_rows(4).desired_width(f32::INFINITY)
                        .hint_text("https://www.youtube.com/watch?v=..."));
                });

                ui.horizontal(|ui| {
                    let mut format = usize::from(self.s.mp3);
                    if w::segmented(ui, "format", &[tr("format_mp4"), tr("format_mp3")], &mut format, half).changed() {
                        self.s.mp3 = format == 1;
                    }
                    ui.add_enabled_ui(!self.s.mp3, |ui| {
                        let mut quality = (self.s.quality as usize).min(settings::QUALITIES.len() - 1);
                        if w::segmented(ui, "quality", &[tr("quality_best"), "1080p", "720p"], &mut quality, half).tip(tr("quality_tip")).changed() {
                            self.s.quality = quality as u8;
                        }
                    });
                });

                ui.horizontal(|ui| {
                    let (field, hover) = ui.allocate_exact_size(vec2(ui.available_width() - 40.0, 32.0), Sense::hover());
                    ui.painter().rect_filled(field, 9, WELL);
                    ui.painter().rect_stroke(field, 9, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
                    ui.painter().with_clip_rect(field.shrink2(vec2(10.0, 0.0)))
                        .text(pos2(field.left() + 10.0, field.center().y), Align2::LEFT_CENTER, &self.s.download_dir, FontId::proportional(13.0), TEXT);
                    hover.tip(self.s.download_dir.clone());
                    if w::icon_button(ui, "folder", tr("folder_tip")).clicked() {
                        let dialog = rfd::FileDialog::new().set_parent(frame).set_title(tr("pick_folder")).set_directory(&self.s.download_dir);
                        if let Some(folder) = dialog.pick_folder() {
                            self.s.download_dir = folder.to_string_lossy().into_owned();
                        }
                    }
                });

                let size = vec2(ui.available_width(), 44.0);
                if busy {
                    if w::action_button(ui, "cancel", tr("cancel_btn"), Kind::Danger, size).tip("Esc").clicked() {
                        self.cancel();
                    }
                } else if w::action_button(ui, "download", tr("download_btn"), Kind::Primary, size).tip("Enter").clicked() {
                    self.start_download(ctx);
                }

                ui.add_space(4.0);
                section(ui, tr("convert_box"));
                let mut picked = None;
                ui.add_enabled_ui(!busy, |ui| {
                    for (kind, icon, tip, labels) in [
                        (converter::Kind::Video, "video", "video_tip", ["video_file_btn", "video_folder_btn"]),
                        (converter::Kind::Image, "image", "image_tip", ["image_file_btn", "image_folder_btn"]),
                    ] {
                        ui.horizontal(|ui| {
                            for (folder, label) in [false, true].into_iter().zip(labels) {
                                let icon = if folder { "folder" } else { icon };
                                if w::action_button(ui, icon, tr(label), Kind::Ghost, vec2(half, 40.0)).tip(tr(tip)).clicked() {
                                    picked = Some((kind, folder));
                                }
                            }
                        });
                    }
                });
                if let Some((kind, folder)) = picked {
                    let sources = self.pick_sources(frame, kind, folder);
                    self.start_conversion(ctx, kind, sources);
                }

                ui.add_space(4.0);
                w::progress_bar(ui, self.progress);
                let color = match self.tone {
                    Tone::Normal => MUTED,
                    Tone::Good => SUCCESS,
                    Tone::Bad => RED,
                };
                ui.add(egui::Label::new(RichText::new(self.status.text()).color(color)).truncate());

                ui.add_space(4.0);
                section(ui, tr("history_label"));
                self.history_list(ui);
            });
    }

    fn history_list(&self, ui: &mut egui::Ui) {
        let frame = egui::Frame::default().fill(WELL).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(9).inner_margin(egui::Margin::same(4));
        frame.show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("history").auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (index, entry) in self.history.iter().enumerate() {
                    let (row, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    if response.hovered() {
                        ui.painter().rect_filled(row, 6, RAISED);
                    }
                    ui.painter().with_clip_rect(row.shrink2(vec2(8.0, 0.0)))
                        .text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, history_label(entry), FontId::proportional(12.5), TEXT);
                    let _ = index;
                    if response.double_clicked() && std::path::Path::new(&entry.path).is_dir() {
                        let _ = proc::command("explorer").arg(&entry.path).spawn();
                    }
                    response.tip(tr("history_tip"));
                }
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

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).size(12.0).color(MUTED));
}

/// "v1.0.0 · 11-25 10:12": package version plus the local time `build.rs` stamped.
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
            if let Some(job) = &self.job {
                job.cancel();
            }
        }
        let (escape, enter) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::Enter)));
        if escape && self.busy() {
            self.cancel();
        }
        // Inside the link box Enter is a line break; anywhere else it starts the download.
        if enter && !self.busy() && ctx.memory(|m| m.focused().is_none()) {
            self.start_download(ctx);
        }
        self.title_bar(ctx);
        self.content(ctx, frame);
        self.resize_grips(ctx);
        self.screenshot_step(ctx);
        if self.shot.is_none() && self.s != self.stored {
            self.s.store();
            self.stored = self.s.clone();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // the rounded window corners stay see-through
    }
}

fn main() -> eframe::Result<()> {
    crashlog::install();
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
    let shot = value("--screenshot").map(PathBuf::from);
    // Screenshots start from defaults so they never depend on, or touch, the user's settings.
    let mut s = if shot.is_some() { Settings::default() } else { Settings::load() };
    if shot.is_some() && value("--lang").is_some_and(|lang| lang == "en") {
        s.language = 1;
    }
    // The first frame must already be in the saved language.
    i18n::set_language(s.language);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size(WINDOW)
        .with_min_inner_size(MIN_WINDOW)
        .with_transparent(true)
        .with_decorations(false)
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/xdownload2.png")).expect("bundled icon"));
    if shot.is_some() {
        viewport = viewport.with_active(false);
    }
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(App::new(cc, s, shot)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_trimmed_and_blank_lines_dropped() {
        assert_eq!(parse_urls("  https://a \r\n\n\thttps://b\n   \n"), ["https://a", "https://b"]);
        assert!(parse_urls(" \n\t\n").is_empty());
    }

    #[test]
    fn batch_summary_reports_counts_and_tone() {
        assert_eq!(summary("done_ok", false, 3, 0), (Status::new("done_ok").with("s", 3), Tone::Good));
        assert_eq!(summary("convert_done_ok", false, 2, 0).0.key, "convert_done_ok");
        assert_eq!(summary("done_ok", false, 1, 2), (Status::new("done_fail").with("s", 1).with("f", 2), Tone::Bad));
        assert_eq!(summary("done_ok", true, 1, 0).1, Tone::Normal);
        assert_eq!(summary("done_ok", true, 1, 1), (Status::new("cancelled").with("s", 1).with("f", 1), Tone::Bad));
    }

    #[test]
    fn every_status_and_label_key_exists_in_the_table() {
        for key in [
            "status_idle", "prep_binaries", "prep_ffmpeg", "downloading", "progress_file", "convert_progress_file", "binary_fail",
            "item_error", "convert_item_error", "retry_update", "cancelling", "cancelled", "done_fail", "done_ok", "convert_done_ok",
            "need_url", "need_convert_files", "url_label", "format_mp4", "format_mp3", "quality_best", "quality_tip", "folder_tip",
            "download_btn", "cancel_btn", "convert_box", "video_file_btn", "video_folder_btn", "image_file_btn", "image_folder_btn",
            "video_tip", "image_tip", "history_label", "history_tip", "pick_folder", "pick_video_files", "pick_video_folder",
            "pick_image_files", "pick_image_folder", "video_files", "image_files", "all_files", "lang_tip", "close", "maximize",
            "minimize", "checksum_missing", "checksum_fail", "zip_no_ffmpeg",
        ] {
            assert!(i18n::TABLE.iter().any(|row| row.0 == key), "missing translation key {key}");
        }
        assert_eq!(history_label(&Entry { url: "u".into(), format: "mp3".into(), when: "2026-11-25 10:00".into(), ..Default::default() }), "[2026-11-25 10:00] MP3 · u");
    }

    #[test]
    fn worker_messages_drive_progress_history_and_the_idle_state() {
        let mut app = App::fresh(Settings::default(), Some(PathBuf::from("unused.png")));
        app.history.clear();
        app.job = Some(Arc::new(Job::default()));
        app.tx.send(Msg::Progress(0.5)).unwrap();
        app.tx.send(Msg::Status(Status::new("downloading").with("i", 1).with("n", 1))).unwrap();
        app.drain();
        assert_eq!((app.progress, app.status.key, app.busy()), (0.5, "downloading", true));
        // After a cancel the "cancelling" line is not overwritten by late worker output.
        app.job.as_ref().unwrap().cancel();
        app.set_status(Status::new("cancelling"), Tone::Normal);
        app.tx.send(Msg::Status(Status::new("progress_file"))).unwrap();
        app.drain();
        assert_eq!(app.status.key, "cancelling");
        app.tx.send(Msg::Done(Status::new("cancelled").with("s", 0).with("f", 0), Tone::Normal)).unwrap();
        app.drain();
        assert_eq!((app.progress, app.status.key, app.busy()), (0.0, "cancelled", false));
        // Starting with no links reports it instead of launching a job.
        app.urls = " \n".into();
        app.start_download(&egui::Context::default());
        assert_eq!((app.status.key, app.tone, app.busy()), ("need_url", Tone::Bad, false));
    }

    /// Manual end-to-end check against the real network (downloads yt-dlp / ffmpeg on first run):
    /// `cargo test --release live_download_and_conversion -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_download_and_conversion() {
        let dir = std::env::temp_dir().join(format!("xdownload2_live_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |work: Box<dyn FnOnce(Arc<Job>, Link)>| -> (Status, Tone, usize) {
            let (tx, rx) = mpsc::channel();
            work(Arc::new(Job::default()), Link { tx, ctx: egui::Context::default() });
            let (mut last, mut recorded) = (None, 0);
            for msg in rx.try_iter() {
                match msg {
                    Msg::Done(status, tone) => last = Some((status, tone)),
                    Msg::Recorded(_) => recorded += 1,
                    Msg::Status(status) => println!("  {}", status.text()),
                    Msg::Progress(_) => {}
                }
            }
            let (status, tone) = last.expect("every batch ends with Done");
            println!("=> {}", status.text());
            (status, tone, recorded)
        };
        let has = |ext: &str| std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == ext)).count();
        let url = "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_owned();
        for (mp3, ext) in [(false, "mp4"), (true, "mp3")] {
            let plan = DownloadPlan { urls: vec![url.clone()], mp3, quality: "720p", out_dir: dir.to_string_lossy().into_owned() };
            let (status, tone, recorded) = run(Box::new(move |job, link| download_worker(plan, job, link)));
            assert_eq!((status.key, tone, recorded, has(ext)), ("done_ok", Tone::Good, 1, 1), "{ext}");
        }
        // A bad link fails without stopping the batch, and only the good one is recorded.
        let plan = DownloadPlan { urls: vec!["https://www.youtube.com/watch?v=xxxxxxxxxxx".into(), url], mp3: true, quality: "best", out_dir: dir.to_string_lossy().into_owned() };
        let (status, tone, recorded) = run(Box::new(move |job, link| download_worker(plan, job, link)));
        assert_eq!((status.text(), tone, recorded), (i18n::fill("done_fail", &[("s", "1"), ("f", "1")]), Tone::Bad, 1));
        image::RgbaImage::from_pixel(64, 48, image::Rgba([200, 60, 20, 255])).save(dir.join("pic.png")).unwrap();
        for (kind, ext, expected) in [(converter::Kind::Image, "jpg", 1), (converter::Kind::Video, "mp4", 2)] {
            let files = converter::collect_media_files(&[dir.clone()], kind);
            assert_eq!(files.len(), 1, "{kind:?}");
            let (status, tone, _) = run(Box::new(move |job, link| convert_worker(kind, files, job, link)));
            assert_eq!((status.key, tone, has(ext)), ("convert_done_ok", Tone::Good, expected), "{kind:?}");
        }
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            println!("{:>10} {}", entry.metadata().unwrap().len(), entry.file_name().to_string_lossy());
            assert!(entry.metadata().unwrap().len() > 0);
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
