//! Glass styling: a painted backdrop with translucent, layered surfaces.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Layout, Margin, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, TextStyle, Ui, Vec2,
};

pub const ACCENT: Color32 = Color32::from_rgb(10, 132, 255);
pub const GREEN: Color32 = Color32::from_rgb(48, 209, 88);
pub const AMBER: Color32 = Color32::from_rgb(255, 179, 64);
pub const RED: Color32 = Color32::from_rgb(255, 69, 58);
pub const TEXT: Color32 = Color32::from_rgb(242, 242, 247);
pub const DIM: Color32 = Color32::from_rgb(150, 150, 162);

const RADIUS_SM: u8 = 10;
const RADIUS_LG: u8 = 14;

/// Fonts and widget style. Call once, before the first frame.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.options_mut(|options| options.theme_preference = egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::dark();
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.panel_fill = Color32::TRANSPARENT;
        style.visuals.window_fill = Color32::from_white_alpha(12);
        style.visuals.extreme_bg_color = Color32::from_black_alpha(90);
        style.visuals.faint_bg_color = Color32::from_white_alpha(8);
        style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
        style.visuals.selection.stroke = Stroke::NONE;
        style.visuals.window_corner_radius = CornerRadius::same(RADIUS_LG);
        style.visuals.menu_corner_radius = CornerRadius::same(RADIUS_SM);

        let widgets = &mut style.visuals.widgets;
        for (widget, fill, hover) in [
            (&mut widgets.inactive, 18, 30),
            (&mut widgets.hovered, 30, 44),
            (&mut widgets.active, 44, 54),
            (&mut widgets.noninteractive, 10, 10),
        ] {
            widget.bg_fill = Color32::from_white_alpha(fill);
            widget.weak_bg_fill = Color32::from_white_alpha(fill);
            widget.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(hover));
            widget.fg_stroke = Stroke::new(1.0, TEXT);
            widget.corner_radius = CornerRadius::same(RADIUS_SM);
            widget.expansion = 0.0;
        }
        widgets.open.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.8));

        style.spacing.item_spacing = Vec2::new(8.0, 7.0);
        style.spacing.button_padding = Vec2::new(11.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(21.0)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();
    });
}

/// Apple's own faces when we're on a Mac, egui's bundled fonts elsewhere.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |path: &str, family: FontFamily, slot: &str| {
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        fonts
            .font_data
            .insert(slot.to_owned(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, slot.to_owned());
    };
    add("/System/Library/Fonts/SFNS.ttf", FontFamily::Proportional, "sf");
    add(
        "/System/Library/Fonts/SFNSMono.ttf",
        FontFamily::Monospace,
        "sf-mono",
    );
    ctx.set_fonts(fonts);
}

/// The wallpaper behind every surface: a soft gradient with two colour glows.
pub fn backdrop(ui: &Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    vertical_gradient(
        painter,
        rect,
        Color32::from_rgb(30, 32, 42),
        Color32::from_rgb(9, 10, 14),
    );
    radial_glow(
        painter,
        rect.left_top() + Vec2::new(rect.width() * 0.2, 30.0),
        rect.width() * 0.8,
        Color32::from_rgba_unmultiplied(70, 120, 255, 58),
    );
    radial_glow(
        painter,
        rect.right_bottom() + Vec2::new(-rect.width() * 0.15, -20.0),
        rect.width() * 0.7,
        Color32::from_rgba_unmultiplied(170, 90, 255, 40),
    );
}

fn vertical_gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(Shape::mesh(mesh));
}

fn radial_glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    const SEGMENTS: u32 = 72;
    if radius <= 0.0 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..=SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, Color32::TRANSPARENT);
    }
    for i in 1..=SEGMENTS {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(Shape::mesh(mesh));
}

/// A frosted panel, with a light catch along its top edge.
pub fn glass<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let panel = Frame::new()
        .fill(Color32::from_white_alpha(14))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(24)))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .inner_margin(Margin::same(12));
    let inner = panel.show(ui, add);
    top_highlight(ui.painter(), inner.response.rect, RADIUS_LG);
    inner.inner
}

/// A recessed well, for output that scrolls.
pub fn well<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let panel = Frame::new()
        .fill(Color32::from_black_alpha(110))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(16)))
        .corner_radius(CornerRadius::same(RADIUS_SM))
        .inner_margin(Margin::same(10));
    panel.show(ui, add).inner
}

fn top_highlight(painter: &egui::Painter, rect: Rect, radius: u8) {
    let line = Rect::from_min_size(
        rect.left_top() + Vec2::new(1.0, 1.0),
        Vec2::new(rect.width() - 2.0, 1.0),
    );
    painter.rect_filled(
        line,
        CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(46),
    );
}

#[derive(Clone, Copy)]
pub enum Kind {
    /// The one thing we want the user to press.
    Primary,
    /// Everything else that is clickable.
    Glass,
    /// Barely there: copy chips, re-detect.
    Quiet,
}

pub fn button(ui: &mut Ui, text: &str, kind: Kind) -> Response {
    let hovered = (Color32::from_white_alpha(34), Color32::from_white_alpha(48));
    let (fill, hover, fg, height) = match kind {
        Kind::Primary => (ACCENT, ACCENT.gamma_multiply(1.15), Color32::WHITE, 30.0),
        Kind::Glass => (
            Color32::from_white_alpha(22),
            hovered.1,
            TEXT,
            30.0,
        ),
        Kind::Quiet => (
            Color32::from_white_alpha(14),
            hovered.0,
            DIM,
            24.0,
        ),
    };
    ui.scope(|ui| {
        let widgets = &mut ui.style_mut().visuals.widgets;
        widgets.inactive.weak_bg_fill = fill;
        widgets.inactive.bg_fill = fill;
        widgets.hovered.weak_bg_fill = hover;
        widgets.hovered.bg_fill = hover;
        widgets.active.weak_bg_fill = hover;
        ui.add(
            egui::Button::new(RichText::new(text).color(fg).size(13.5))
                .corner_radius(CornerRadius::same(RADIUS_SM))
                .min_size(Vec2::new(0.0, height)),
        )
    })
    .inner
}

/// iOS-style switch — a checkbox reads as a settings form, this reads as a control.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::click());
        if response.clicked() {
            *on = !*on;
        }
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let track = Color32::from_white_alpha(40).lerp_to_gamma(GREEN, t);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(11), track);
        ui.painter().circle_filled(
            Pos2::new(
                egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t),
                rect.center().y,
            ),
            8.5,
            Color32::WHITE,
        );
        if !label.is_empty() {
            ui.label(label);
        }
        response
    })
    .inner
}

/// A coloured capsule label, for state.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(11.5), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(20.0, 7.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(11), color.gamma_multiply(0.2));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Monospace line the user can select or copy by hand; wraps rather than
/// pushing the copy chip off the edge.
pub fn mono_row(ui: &mut Ui, text: &str) {
    ui.add(
        egui::Label::new(RichText::new(text).monospace().color(TEXT))
            .selectable(true)
            .wrap_mode(egui::TextWrapMode::Wrap),
    );
}

/// Sits in the top-right corner of the window: the app's state at a glance.
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        pill(ui, text, color);
    });
}
