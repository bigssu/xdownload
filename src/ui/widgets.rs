//! Custom-painted controls shared by the whole window (ported from Xgif2).
use super::{icons, theme::*};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::hash::Hash;

/// CSS cubic-bezier(.3, 1.3, .4, 1), solved in the time axis: a short overshoot, then settle.
pub fn spring(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let (mut lo, mut hi): (f32, f32) = (0.0, 1.0);
    for _ in 0..18 {
        let u = (lo + hi) * 0.5;
        let x = 3.0 * (1.0 - u).powi(2) * u * 0.3 + 3.0 * (1.0 - u) * u * u * 0.4 + u * u * u;
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
    }
    let u = (lo + hi) * 0.5;
    3.0 * (1.0 - u).powi(2) * u * 1.3 + 3.0 * (1.0 - u) * u * u + u * u * u
}

/// Left edge and width of the selected cell inside a segmented track (3 px inset).
pub fn segment_geometry(width: f32, count: usize, selected: usize) -> (f32, f32) {
    if count == 0 {
        return (3.0, 0.0);
    }
    let cell = (width - 6.0).max(0.0) / count as f32;
    (3.0 + selected.min(count - 1) as f32 * cell, cell)
}

/// A rounded rectangle with a continuous diagonal gradient from `a` to `b`.
pub fn paint_gradient(p: &egui::Painter, r: Rect, radius: f32, a: Color32, b: Color32) {
    let radius = radius.min(r.width() * 0.5).min(r.height() * 0.5).max(0.0);
    let color = |pt: egui::Pos2| {
        let t = (((pt.x - r.left()) / r.width().max(1.0) + (pt.y - r.top()) / r.height().max(1.0)) * 0.5).clamp(0.0, 1.0);
        Color32::from_rgba_unmultiplied(
            egui::lerp(a.r() as f32..=b.r() as f32, t) as u8,
            egui::lerp(a.g() as f32..=b.g() as f32, t) as u8,
            egui::lerp(a.b() as f32..=b.b() as f32, t) as u8,
            egui::lerp(a.a() as f32..=b.a() as f32, t) as u8,
        )
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.center(), color(r.center()));
    for (center, start) in [
        (pos2(r.right() - radius, r.top() + radius), -std::f32::consts::FRAC_PI_2),
        (pos2(r.right() - radius, r.bottom() - radius), 0.0),
        (pos2(r.left() + radius, r.bottom() - radius), std::f32::consts::FRAC_PI_2),
        (pos2(r.left() + radius, r.top() + radius), std::f32::consts::PI),
    ] {
        for n in 0..=6 {
            let angle = start + n as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
            let pt = center + vec2(angle.cos(), angle.sin()) * radius;
            mesh.colored_vertex(pt, color(pt));
        }
    }
    for i in 1..28 {
        mesh.add_triangle(0, i, i + 1);
    }
    mesh.add_triangle(0, 28, 1);
    p.add(egui::Shape::mesh(mesh));
}

/// Hover lift and press squeeze shared by every button.
fn motion(ui: &Ui, response: &Response) -> Rect {
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.18);
    let press = ui.ctx().animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), 0.12);
    response.rect.translate(vec2(0.0, -2.0 * spring(hover))).shrink2(response.rect.size() * press * 0.025)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// The main action: accent gradient.
    Primary,
    /// Cancel: red gradient.
    Danger,
    /// Secondary actions: flat raised surface.
    Ghost,
}

/// A button of exactly `size` with an icon followed by a label.
pub fn action_button(ui: &mut Ui, icon: &str, label: &str, kind: Kind, size: Vec2) -> Response {
    let (_, response) = ui.allocate_exact_size(size, Sense::click());
    let r = motion(ui, &response);
    let p = ui.painter();
    match kind {
        Kind::Ghost => {
            p.rect_filled(r, 12, if response.hovered() { HOVER } else { RAISED });
            p.rect_stroke(r, 12, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
        }
        _ => {
            let (base, top, bottom) = if kind == Kind::Danger { (RED, RED_TOP, RED_BOTTOM) } else { (ACCENT, ACCENT, ACCENT_END) };
            // Soft glow under the button.
            p.rect_filled(r.translate(vec2(0.0, 4.0)).expand(2.0), 14, base.gamma_multiply(0.13));
            paint_gradient(p, r, 12.0, top, bottom);
        }
    }
    let ink = if kind == Kind::Ghost { TEXT } else { Color32::WHITE };
    let galley = p.layout_no_wrap(label.to_owned(), FontId::proportional(13.5), ink);
    let icon_size = 18.0;
    let total = icon_size + 8.0 + galley.size().x;
    let left = r.center().x - total / 2.0;
    icons::paint(p, Rect::from_min_size(pos2(left, r.center().y - icon_size / 2.0), Vec2::splat(icon_size)), icon, ink);
    p.galley(pos2(left + icon_size + 8.0, r.center().y - galley.size().y / 2.0), galley, ink);
    response
}

/// A 32 px icon-only button; the tooltip is its name, so it is required.
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    debug_assert!(!tooltip.trim().is_empty(), "Icon-only controls require a tooltip");
    let (_, response) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let r = motion(ui, &response);
    let hover = ui.ctx().animate_bool_with_time(response.id.with("fill"), response.hovered(), 0.15);
    ui.painter().rect_filled(r, 9, if icon == "close" { RED.gamma_multiply(hover) } else { RAISED.gamma_multiply(hover) });
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(17.0, 17.0)), icon, if response.hovered() { TEXT } else { MUTED });
    response.tip(tooltip)
}

/// A track of text options with a sliding light pill under the chosen one.
pub fn segmented(ui: &mut Ui, id_source: impl Hash, labels: &[&str], selected: &mut usize, width: f32) -> Response {
    let (r, mut response) = ui.allocate_exact_size(vec2(width, 34.0), Sense::hover());
    let id = ui.make_persistent_id(id_source);
    ui.painter().rect_filled(r, 11, RAISED);
    ui.painter().rect_stroke(r, 11, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    let (_, cell) = segment_geometry(width, labels.len(), *selected);
    for i in 0..labels.len() {
        let cell_rect = Rect::from_min_size(r.min + vec2(3.0 + i as f32 * cell, 3.0), vec2(cell, 28.0));
        if ui.interact(cell_rect, id.with(i), Sense::click()).clicked() && *selected != i {
            *selected = i;
            response.mark_changed();
        }
    }
    let (left, _) = segment_geometry(width, labels.len(), *selected);
    // (selection, slide start, current position) so a change animates from where the pill is.
    let motion_id = id.with("indicator");
    let mut state = ui.ctx().data_mut(|data| data.get_temp::<(usize, f32, f32)>(motion_id)).unwrap_or((*selected, left, left));
    if state.0 != *selected {
        state = (*selected, state.2, state.2);
        ui.ctx().animate_value_with_time(motion_id, 0.0, 0.0);
    }
    let progress = ui.ctx().animate_value_with_time(motion_id, 1.0, 0.32);
    let x = egui::lerp(state.1..=left, spring(progress));
    state.2 = x;
    ui.ctx().data_mut(|data| data.insert_temp(motion_id, state));
    ui.painter().rect_filled(Rect::from_min_size(r.min + vec2(x, 3.0), vec2(cell, 28.0)), 8, SELECTED);
    for (i, label) in labels.iter().enumerate() {
        let center = pos2(r.left() + 3.0 + (i as f32 + 0.5) * cell, r.center().y);
        ui.painter().text(center, Align2::CENTER_CENTER, *label, FontId::proportional(13.0), if i == *selected { ON_SELECTED } else { MUTED });
    }
    response
}

/// A full-width progress bar; `fraction` is 0..=1.
pub fn progress_bar(ui: &mut Ui, fraction: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(r, 4, WELL);
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let fill = Rect::from_min_size(r.min, vec2((r.width() * fraction).max(r.height()), r.height()));
        paint_gradient(ui.painter(), fill, 4.0, ACCENT, ACCENT_END);
    }
}

/// App tooltip: an inverted (light) bubble centred above the widget, flipped below only when
/// it would leave the window.
pub trait Tip {
    fn tip(self, text: impl Into<String>) -> Self;
}

impl Tip for Response {
    fn tip(self, text: impl Into<String>) -> Self {
        if self.hovered() && !self.ctx.input(|i| i.pointer.any_down()) {
            let text = text.into();
            if !text.trim().is_empty() && tip_due(&self.ctx, self.id) {
                show_tip(&self.ctx, self.id, self.rect, &text);
            }
        }
        self
    }
}

/// Hover delay with a warm window: moving between neighbouring controls shows tips at once.
/// One tooltip per frame: the first hovered widget to ask owns it.
fn tip_due(ctx: &egui::Context, id: egui::Id) -> bool {
    const DELAY: f64 = 0.4;
    const WARM: f64 = 0.6;
    let key = egui::Id::new("tip_state");
    let now = ctx.input(|i| i.time);
    let pass = ctx.cumulative_pass_nr();
    let (current, start, last, owner_pass) =
        ctx.data(|d| d.get_temp::<(egui::Id, f64, f64, u64)>(key)).unwrap_or((egui::Id::NULL, now, f64::MIN, u64::MAX));
    if owner_pass == pass {
        return false;
    }
    let start = if current == id { start } else { now };
    let due = now - last < WARM || now - start >= DELAY;
    let last = if due { now } else { last };
    ctx.data_mut(|d| d.insert_temp(key, (id, start, last, pass)));
    if !due {
        ctx.request_repaint_after_secs((DELAY - (now - start)).max(0.0) as f32);
    }
    due
}

/// Where the bubble goes: centred above `anchor`, below it only if above would leave `screen`,
/// and slid sideways to stay on screen. Returns the bubble rect and whether it sits above.
pub fn tip_rect(screen: Rect, anchor: Rect, size: Vec2) -> (Rect, bool) {
    const GAP: f32 = 8.0;
    const EDGE: f32 = 4.0;
    let above = anchor.top() - GAP - size.y >= screen.top() + EDGE;
    let top = if above { anchor.top() - GAP - size.y } else { anchor.bottom() + GAP };
    let max_x = (screen.right() - EDGE - size.x).max(screen.left() + EDGE);
    let x = (anchor.center().x - size.x / 2.0).clamp(screen.left() + EDGE, max_x);
    (Rect::from_min_size(pos2(x, top), size), above)
}

fn show_tip(ctx: &egui::Context, id: egui::Id, anchor: Rect, text: &str) {
    let galley = ctx.fonts(|f| f.layout(text.to_owned(), FontId::proportional(12.5), TIP_TEXT, 300.0));
    let pad = vec2(10.0, 6.0);
    let (r, above) = tip_rect(ctx.screen_rect(), anchor, galley.size() + pad * 2.0);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, id.with("tip")));
    painter.add(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(120) }.as_shape(r, 7));
    painter.rect_filled(r, 7, TIP_BG);
    // Caret pointing at the widget.
    let cx = anchor.center().x.clamp(r.left() + 10.0, r.right() - 10.0);
    let (base, point) = if above { (r.bottom(), r.bottom() + 5.0) } else { (r.top(), r.top() - 5.0) };
    painter.add(egui::Shape::convex_polygon(vec![pos2(cx - 6.0, base), pos2(cx + 6.0, base), pos2(cx, point)], TIP_BG, Stroke::NONE));
    painter.galley(r.min + pad, galley, TIP_TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs one frame per entry of `pointer` (position, pressed/released) and returns nothing;
    /// `body` records what it needs.
    fn frames(ctx: &egui::Context, count: usize, target: impl Fn() -> egui::Pos2, mut body: impl FnMut(&mut Ui)) {
        for frame in 0..count {
            let mut input = egui::RawInput { time: Some(frame as f64 * 0.1), ..Default::default() };
            if frame > 0 {
                let pos = target();
                input.events.push(egui::Event::PointerMoved(pos));
                if frame == 1 || frame == 2 {
                    input.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: frame == 1, modifiers: egui::Modifiers::NONE });
                }
            }
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| body(ui));
            });
        }
    }

    #[test]
    fn action_button_keeps_its_size_and_clicks_only_when_enabled() {
        for enabled in [false, true] {
            let ctx = egui::Context::default();
            let rect = std::cell::Cell::new(Rect::NOTHING);
            let mut clicks = 0;
            frames(&ctx, 4, || rect.get().center(), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let response = action_button(ui, "download", "Download", Kind::Primary, vec2(200.0, 40.0));
                    rect.set(response.rect);
                    clicks += usize::from(response.clicked());
                });
            });
            assert_eq!(rect.get().size(), vec2(200.0, 40.0));
            assert_eq!(clicks, usize::from(enabled));
        }
    }

    #[test]
    fn segmented_selects_the_clicked_cell_and_reports_one_change() {
        let ctx = egui::Context::default();
        let track = std::cell::Cell::new(Rect::NOTHING);
        let (mut selected, mut changes) = (0usize, 0);
        // The third of three cells.
        frames(&ctx, 4, || pos2(track.get().left() + track.get().width() * 5.0 / 6.0, track.get().center().y), |ui| {
            let response = segmented(ui, "quality", &["a", "b", "c"], &mut selected, 198.0);
            track.set(response.rect);
            changes += usize::from(response.changed());
        });
        assert_eq!((selected, changes), (2, 1));
    }

    #[test]
    fn segmented_geometry_stays_inside_track() {
        assert_eq!(segment_geometry(198.0, 3, 0), (3.0, 64.0));
        assert_eq!(segment_geometry(198.0, 3, 2), (131.0, 64.0));
        assert_eq!(segment_geometry(198.0, 3, 99), (131.0, 64.0));
        assert_eq!(segment_geometry(0.0, 0, 0), (3.0, 0.0));
    }

    #[test]
    fn spring_has_exact_endpoints_and_overshoot() {
        assert_eq!(spring(0.0), 0.0);
        assert_eq!(spring(1.0), 1.0);
        assert!(spring(0.7) > 1.0);
        assert_eq!(spring(-1.0), 0.0);
    }

    #[test]
    fn tooltips_sit_above_and_flip_below_only_at_the_top_edge() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let size = vec2(80.0, 24.0);
        let icon = Rect::from_min_size(pos2(300.0, 300.0), vec2(40.0, 40.0));
        let (r, above) = tip_rect(screen, icon, size);
        assert!(above && r.bottom() < icon.top() && (r.center().x - icon.center().x).abs() < 0.01);
        let top_icon = Rect::from_min_size(pos2(300.0, 4.0), vec2(40.0, 40.0));
        let (r, above) = tip_rect(screen, top_icon, size);
        assert!(!above && r.top() > top_icon.bottom());
        let edge_icon = Rect::from_min_size(pos2(770.0, 300.0), vec2(30.0, 30.0));
        assert!(tip_rect(screen, edge_icon, size).0.right() <= screen.right());
    }

    #[test]
    #[should_panic(expected = "Icon-only controls require a tooltip")]
    fn icon_controls_reject_empty_tooltips() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                icon_button(ui, "close", "");
            });
        });
    }
}
