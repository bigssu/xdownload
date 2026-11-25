//! The four tab screens and the shared work list.
//!
//! Icon first: text is kept only for what an icon cannot say (format names, sizes, paths,
//! titles, numbers, error messages). Every icon explains itself in a bubble tip on hover.
use crate::converter::{Encoder, Target};
use crate::i18n::{self, tr};
use crate::queue::{Class, Item, Note, State, Work};
use crate::settings::{self, HEIGHTS};
use crate::ui::icons;
use crate::ui::theme::{self, LINE, MUTED, PANEL, RAISED, RED, SUCCESS, TEXT, WELL};
use crate::ui::widgets::{self as w, Kind, Tip};
use crate::{build_version, open, reveal, App, Tab};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

const ROW_H: f32 = 46.0;

/// A small icon that only carries a bubble tip (marks what a row of controls is about).
fn marker(ui: &mut egui::Ui, icon: &str, tooltip: &str) {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 34.0), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, tooltip));
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(19.0, 19.0)), icon, MUTED);
    response.tip(tooltip);
}

/// A sunken, rounded list area that fills the rest of the tab.
fn list_frame(ui: &mut egui::Ui, id: &str, empty: Option<&str>, body: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::default().fill(WELL).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(9).inner_margin(egui::Margin::same(4));
    frame.show(ui, |ui| {
        if let Some(text) = empty {
            let rect = ui.available_rect_before_wrap();
            ui.allocate_rect(rect, Sense::hover());
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(13.0), MUTED);
            return;
        }
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            body(ui);
        });
    });
}

/// One list row: icon, title, an optional smaller second line, and room on the right for
/// `buttons` (26 px each). Returns the row rect and its click response.
fn row(ui: &mut egui::Ui, icon: (&str, egui::Color32), title: &str, sub: (&str, egui::Color32), buttons: usize) -> (Rect, egui::Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 7, RAISED);
    }
    icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(18.0, 18.0)), icon.0, icon.1);
    let text_area = Rect::from_min_max(pos2(rect.left() + 40.0, rect.top()), pos2(rect.right() - 12.0 - buttons as f32 * 30.0, rect.bottom()));
    let painter = ui.painter().with_clip_rect(text_area);
    if sub.0.is_empty() {
        painter.text(pos2(text_area.left(), rect.center().y), Align2::LEFT_CENTER, title, FontId::proportional(13.0), TEXT);
    } else {
        painter.text(pos2(text_area.left(), rect.top() + 14.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), TEXT);
        painter.text(pos2(text_area.left(), rect.top() + 32.0), Align2::LEFT_CENTER, sub.0, FontId::proportional(11.5), sub.1);
    }
    (rect, response)
}

/// A right-aligned strip inside `rect` for a row's icon buttons.
fn row_buttons(ui: &mut egui::Ui, rect: Rect, body: impl FnOnce(&mut egui::Ui)) {
    let area = rect.shrink2(vec2(8.0, 10.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 4.0;
    body(&mut child);
}

/// The second line of a work-list row: numbers and messages only. The state itself is the
/// row's icon (and its tip).
fn item_detail(item: &Item) -> (String, egui::Color32) {
    match (&item.state, &item.note) {
        (State::Failed, Note::Error(error)) => (error.clone(), RED),
        (State::Done, Note::Encoder(name)) => (name.to_string(), MUTED),
        (State::Running, _) if item.cancelling() => (String::new(), MUTED),
        (State::Running, note) => {
            let percent = format!("{:.0}%", item.progress * 100.0);
            let text = match note {
                Note::Preparing => format!("{} · {percent}", tr("st_preparing")),
                Note::Retrying => tr("st_retrying").into(),
                Note::Speed { speed, eta } => [percent.as_str(), speed, eta].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · "),
                Note::Encoder(name) => format!("{percent} · {name}"),
                Note::None | Note::Error(_) => percent,
            };
            (text, MUTED)
        }
        _ => (String::new(), MUTED),
    }
}

/// Icon, colour and tip for an item's state.
fn item_state(item: &Item) -> (&'static str, egui::Color32, &'static str) {
    if item.cancelling() {
        return ("stopped", MUTED, "st_cancelling");
    }
    match item.state {
        State::Done => ("done", SUCCESS, "st_done"),
        State::Failed => ("failed", RED, "st_failed_tip"),
        State::Cancelled => ("stopped", MUTED, "st_cancelled"),
        State::Waiting => ("waiting", MUTED, "st_waiting"),
        State::Running => (match item.work.class() {
            Class::Download => "download",
            Class::Video => "video",
            Class::Image => "image",
        }, theme::ACCENT, "st_running"),
    }
}

enum Act {
    Cancel(u64),
    Retry(u64),
    Remove(u64),
    Reveal(std::path::PathBuf),
    CancelAll,
    ClearFinished,
}

impl App {
    pub(crate) fn content(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::same(18)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
                match self.tab {
                    Tab::Download => {
                        self.download_tab(ui, frame);
                        self.work_list(ui);
                    }
                    Tab::Convert => {
                        self.convert_tab(ui, frame);
                        self.work_list(ui);
                    }
                    Tab::History => self.history_tab(ui),
                    Tab::Settings => self.settings_tab(ui),
                }
            });
    }

    fn download_tab(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        egui::ScrollArea::vertical().id_salt("urls").max_height(92.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.urls).desired_rows(4).desired_width(f32::INFINITY).hint_text(tr("url_label")));
        });

        let mp4 = self.s.format == 0;
        ui.horizontal(|ui| {
            let mut format = (self.s.format as usize).min(2);
            if w::segmented_tips(ui, "format", &[("MP4", tr("tip_mp4")), ("MP3", tr("tip_mp3")), ("M4A", tr("tip_m4a"))], &mut format, 168.0).changed() {
                self.s.format = format as u8;
            }
            ui.add_enabled_ui(mp4, |ui| {
                let mut quality = HEIGHTS.iter().position(|height| *height == self.s.height).unwrap_or(0);
                let width = ui.available_width() - 3.0 * 52.0;
                let cells = [("best", tr("tip_best")), ("4K", ""), ("1440p", ""), ("1080p", ""), ("720p", ""), ("480p", "")];
                if w::segmented_tips(ui, "quality", &cells, &mut quality, width).changed() {
                    self.s.height = HEIGHTS[quality];
                }
                w::chip(ui, &mut self.s.subs, "subs", &format!("{} — {}", tr("opt_subs"), tr("opt_subs_tip")));
            });
            w::chip(ui, &mut self.s.embed_info, "info", &format!("{} — {}", tr("opt_info"), tr("opt_info_tip")));
            w::chip(ui, &mut self.s.playlist, "playlist", &format!("{} — {}", tr("opt_playlist"), tr("opt_playlist_tip")));
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
        if w::action_icon(ui, "download", tr("tip_download"), Kind::Primary, size).clicked() {
            self.add_downloads();
        }
        ui.add_space(2.0);
    }

    fn convert_tab(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let half = (ui.available_width() - 8.0) / 2.0;
        // Marker + three equal tracks + the encoder mark.
        let third = (ui.available_width() - 24.0 - 28.0 - 4.0 * 8.0) / 3.0;
        let mut picked = None;

        let mp4 = self.s.video_target == 0;
        ui.horizontal(|ui| {
            marker(ui, "video", tr("conv_video"));
            let mut target = (self.s.video_target as usize).min(2);
            if w::segmented_tips(ui, "video_target", &[("MP4", tr("tip_to_mp4")), ("MP3", tr("tip_to_mp3")), ("GIF", tr("tip_to_gif"))], &mut target, third).changed() {
                self.s.video_target = target as u8;
            }
            ui.add_enabled_ui(mp4, |ui| {
                const SIZES: [u32; 3] = [0, 1080, 720];
                let mut size = SIZES.iter().position(|height| *height == self.s.video_height).unwrap_or(1);
                if w::segmented_tips(ui, "video_height", &[("source", tr("tip_source")), ("1080p", tr("size_tip")), ("720p", tr("size_tip"))], &mut size, third).changed() {
                    self.s.video_height = SIZES[size];
                }
                let mut quality = (self.s.video_quality as usize).min(2);
                let cells = [("quality", tr("tip_vq_quality")), ("balanced", tr("tip_vq_balanced")), ("small", tr("tip_vq_small"))];
                if w::segmented_tips(ui, "video_quality", &cells, &mut quality, third).changed() {
                    self.s.video_quality = quality as u8;
                }
            });
            // Which encoder MP4 conversions will use, once known.
            let encoder = if self.s.gpu { self.encoder } else { Some(Encoder::Cpu) };
            if let Some(encoder) = encoder {
                marker(ui, if encoder == Encoder::Cpu { "cpu" } else { "gpu" }, &i18n::fill("encoder_label", &[("name", encoder.label())]));
            }
        });
        ui.horizontal(|ui| {
            for (folder, icon, tip) in [(false, "files", "tip_video_file"), (true, "folder", "tip_video_folder")] {
                if w::action_icon(ui, icon, &format!("{} — {}", tr(tip), tr("tip_drop")), Kind::Ghost, vec2(half, 40.0)).clicked() {
                    picked = Some((self.video_target(), folder));
                }
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            marker(ui, "image", tr("conv_image"));
            let mut target = (self.s.image_target as usize).min(2);
            if w::segmented_tips(ui, "image_target", &[("JPG", tr("tip_to_jpg")), ("PNG", tr("tip_to_png")), ("WebP", tr("tip_to_webp"))], &mut target, third).changed() {
                self.s.image_target = target as u8;
            }
            w::chip(ui, &mut self.s.subfolders, "subfolders", tr("subfolders"));
        });
        ui.horizontal(|ui| {
            // A separate id scope: these two buttons sit under the same parent as the video pair.
            ui.push_id("image_buttons", |ui| {
                for (folder, icon, tip) in [(false, "files", "tip_image_file"), (true, "folder", "tip_image_folder")] {
                    if w::action_icon(ui, icon, &format!("{} — {}", tr(tip), tr("tip_drop")), Kind::Ghost, vec2(half, 40.0)).clicked() {
                        picked = Some((self.image_target(), folder));
                    }
                }
            });
        });
        ui.add_space(2.0);

        if let Some((target, folder)) = picked {
            self.convert_picked(frame, target, folder);
        }
    }

    fn convert_picked(&mut self, frame: &eframe::Frame, target: Target, folder: bool) {
        let sources = self.pick_sources(frame, target, folder);
        if sources.is_empty() {
            return;
        }
        let added = self.add_conversions(&sources, target);
        self.report_added(added, "need_convert_files");
    }

    /// Counts and the list of queued, running and finished work.
    fn work_list(&mut self, ui: &mut egui::Ui) {
        let mut acts = Vec::new();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            w::icon_count(ui, "running", self.queue.count(State::Running), tr("tip_running"));
            w::icon_count(ui, "waiting", self.queue.count(State::Waiting), tr("tip_waiting"));
            w::icon_count(ui, "done", self.queue.count(State::Done), tr("tip_done"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_enabled_ui(self.queue.items.iter().any(Item::finished), |ui| {
                    if w::icon_button_sized(ui, "sweep", tr("clear_done"), 26.0).clicked() {
                        acts.push(Act::ClearFinished);
                    }
                });
                ui.add_enabled_ui(self.queue.active(), |ui| {
                    if w::icon_button_sized(ui, "stopped", tr("cancel_all"), 26.0).clicked() {
                        acts.push(Act::CancelAll);
                    }
                });
            });
        });
        let empty = self.queue.items.is_empty().then(|| tr("queue_empty"));
        list_frame(ui, "queue", empty, |ui| {
            for item in &self.queue.items {
                ui.push_id(item.id, |ui| {
                    let (detail, color) = item_detail(item);
                    let (icon, icon_color, state_tip) = item_state(item);
                    let buttons = if item.finished() { 2 } else { 1 };
                    let (rect, response) = row(ui, (icon, icon_color), &item.title, (&detail, color), buttons);
                    if item.state == State::Running {
                        let bar = Rect::from_min_max(pos2(rect.left() + 40.0, rect.bottom() - 4.0), pos2(rect.right() - 12.0, rect.bottom() - 1.0));
                        ui.painter().rect_filled(bar, 2, RAISED);
                        ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * item.progress, bar.height())), 2, theme::ACCENT);
                    }
                    if let (true, Some(file)) = (response.double_clicked(), &item.file) {
                        open(&file.to_string_lossy());
                    }
                    response.tip(tr(state_tip));
                    row_buttons(ui, rect, |ui| match item.state {
                        State::Waiting | State::Running => {
                            if !item.cancelling() && w::icon_button_sized(ui, "cancel", tr("act_cancel"), 26.0).clicked() {
                                acts.push(Act::Cancel(item.id));
                            }
                        }
                        State::Done => {
                            if w::icon_button_sized(ui, "cancel", tr("act_remove"), 26.0).clicked() {
                                acts.push(Act::Remove(item.id));
                            }
                            if let Some(file) = &item.file {
                                if w::icon_button_sized(ui, "folder", tr("act_open_folder"), 26.0).clicked() {
                                    acts.push(Act::Reveal(file.clone()));
                                }
                            }
                        }
                        State::Failed | State::Cancelled => {
                            if w::icon_button_sized(ui, "cancel", tr("act_remove"), 26.0).clicked() {
                                acts.push(Act::Remove(item.id));
                            }
                            if w::icon_button_sized(ui, "retry", tr("act_retry"), 26.0).clicked() {
                                acts.push(Act::Retry(item.id));
                            }
                        }
                    });
                });
            }
        });
        for act in acts {
            match act {
                Act::Cancel(id) => self.queue.cancel(id),
                Act::Retry(id) => self.queue.retry(id),
                Act::Remove(id) => self.queue.remove(id),
                Act::Reveal(file) => reveal(&file),
                Act::CancelAll => self.queue.cancel_all(),
                Act::ClearFinished => self.queue.clear_finished(),
            }
        }
    }

    /// Draws every screen body into one `ui`, so a test can read all the text.
    #[cfg(test)]
    pub(crate) fn draw_everything(&mut self, ui: &mut egui::Ui) {
        // Buttons that would open a file dialog are never clicked here, so no frame is needed.
        self.work_list(ui);
        self.history_tab(ui);
        self.settings_tab(ui);
    }

    fn history_tab(&mut self, ui: &mut egui::Ui) {
        // (index, 0 = open folder, 1 = download again, 2 = delete)
        let mut act = None;
        let empty = self.history.is_empty().then(|| tr("history_empty"));
        list_frame(ui, "history", empty, |ui| {
            for (index, entry) in self.history.iter().enumerate() {
                ui.push_id(index, |ui| {
                    let title = if entry.title.is_empty() { &entry.url } else { &entry.title };
                    let sub = format!("{}   {} {}   {}", entry.when, entry.format.to_uppercase(), entry.quality, entry.url);
                    let icon = if entry.format == "mp4" { "film" } else { "music" };
                    let (rect, response) = row(ui, (icon, MUTED), title, (&sub, MUTED), 3);
                    if response.double_clicked() {
                        // The file may have been moved or deleted since; fall back to its folder.
                        open(if std::path::Path::new(&entry.file).is_file() { &entry.file } else { &entry.path });
                    }
                    response.tip(tr("history_tip"));
                    row_buttons(ui, rect, |ui| {
                        for (code, icon, tip) in [(2, "trash", "act_delete"), (1, "retry", "act_redo"), (0, "folder", "act_open_folder")] {
                            if w::icon_button_sized(ui, icon, tr(tip), 26.0).clicked() {
                                act = Some((index, code));
                            }
                        }
                    });
                });
            }
        });
        let Some((index, code)) = act else { return };
        let entry = self.history[index].clone();
        match code {
            0 if std::path::Path::new(&entry.file).is_file() => reveal(std::path::Path::new(&entry.file)),
            0 => open(&entry.path),
            1 => {
                // Same link, the options chosen now.
                let options = self.options();
                let added = usize::from(self.queue.add(Work::Download { url: entry.url, options }).is_some());
                self.tab = Tab::Download;
                self.report_added(added, "already");
            }
            _ => {
                self.history.remove(index);
                if self.shot.is_none() {
                    settings::store_history(&self.history);
                }
            }
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 14.0;
        ui.horizontal(|ui| {
            marker(ui, "parallel", tr("set_parallel"));
            let mut parallel = self.s.parallel() - 1;
            let cells = ["1", "2", "3", "4", "5"].map(|count| (count, tr("set_parallel")));
            if w::segmented_tips(ui, "parallel", &cells, &mut parallel, 180.0).changed() {
                self.s.parallel = parallel as u8 + 1;
            }
        });
        ui.horizontal(|ui| {
            w::chip(ui, &mut self.s.gpu, "gpu", &format!("{} — {}", tr("set_gpu"), tr("set_gpu_tip")));
            w::chip(ui, &mut self.s.auto_paste, "paste", tr("set_autopaste"));
            w::chip(ui, &mut self.s.open_when_done, "open", tr("set_open_done"));
            if w::action_icon(ui, "folder", &format!("{} — {}", tr("set_data"), tr("set_data_tip")), Kind::Ghost, vec2(44.0, 34.0)).clicked() {
                if let Some(dir) = settings::app_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    open(&dir.to_string_lossy());
                }
            }
        });
        // Names and numbers stay as text: the encoder found, the version, a newer version.
        let encoder = match (self.s.gpu, self.encoder) {
            (true, Some(encoder)) => format!("   {}", encoder.label()),
            _ => String::new(),
        };
        ui.label(RichText::new(format!("Xdownload2 {}{encoder}", build_version())).size(12.0).color(MUTED));
        if let Some(release) = self.release.clone() {
            let tip = i18n::fill("update_available", &[("v", &release.version)]);
            if w::action_button(ui, "update", &release.version, Kind::Primary, vec2(120.0, 38.0)).tip(tip).clicked() {
                open(&release.url);
            }
        }
    }
}
