// The window's look: the game's own interface (its menu), drawn here from nothing. A dusk sky (violet down to
// peach), a dark band down the middle, framed dark panels, pill buttons with a metal rim and a ball (gold on the one
// under the cursor), labels in the game's interface face (Tektur, bold, lower case as small capitals, spaced).
// Nothing is read from the player's copy and no picture is shipped: the shapes are egui's, the colours are measured
// on the game's own menu art. The fonts are the game's (its assets/fonts, SIL OFL): Tektur and Noto Sans are built
// in, so the first window already looks right; Chinese comes from the game's Noto Sans SC beside the program, else
// from a font the system has (never downloaded).

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};
use std::path::Path;
use std::sync::Arc;

/// The game's interface face and its body face (the game's assets/fonts; the licences lie beside them there).
const TEKTUR: &[u8] = include_bytes!("../../drahtseilakt/assets/fonts/Tektur-Variable.ttf");
const NOTO: &[u8] = include_bytes!("../../drahtseilakt/assets/fonts/NotoSans-Regular.ttf");
/// Tektur's weight, as the game sets it; the small capitals' size against the capitals (the game's 0.8); the space
/// between letters as a fraction of the size.
const WEIGHT: f32 = 700.0;
const SMALL_CAPS: f32 = 0.8;
const TRACKING: f32 = 0.09;
/// The font family of headings and buttons.
const CAPS_FAMILY: &str = "tektur";
/// The largest font file read.
const FONT_MAX: u64 = 64 * 1024 * 1024;

/// Fonts that cover Chinese, where systems keep them (Linux, Windows, macOS); the first one there is used.
#[cfg(windows)]
const CJK_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\simsun.ttc",
];
#[cfg(target_os = "macos")]
const CJK_FONTS: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
];
#[cfg(not(any(windows, target_os = "macos")))]
const CJK_FONTS: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
];

// The colours, measured on the game's menu (its sky and its own button art).
const SKY_TOP: Color32 = Color32::from_rgb(122, 122, 172);
const SKY_MID: Color32 = Color32::from_rgb(168, 154, 168);
const SKY_BOTTOM: Color32 = Color32::from_rgb(206, 174, 160);
pub const TEXT: Color32 = Color32::from_rgb(238, 234, 242);
pub const TEXT_WEAK: Color32 = Color32::from_rgb(184, 178, 198);
pub const GOLD: Color32 = Color32::from_rgb(244, 190, 104);
pub const ERROR: Color32 = Color32::from_rgb(255, 150, 136);
const METAL: Color32 = Color32::from_rgb(178, 176, 196);
const METAL_DIM: Color32 = Color32::from_rgb(133, 122, 149);
const EDGE: Color32 = Color32::from_rgb(34, 33, 40);

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// A font file: a plain one, not too big.
fn read_font(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    // the system's fonts are often links (fontconfig): followed, but the file read is a plain one through a cap
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(FONT_MAX.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? <= FONT_MAX).then_some(bytes)
}

/// The font for Chinese: the game's own beside the program, else the first the system has.
fn chinese_font() -> Option<Vec<u8>> {
    gamedata::resources::assets_root()
        .and_then(|a| read_font(&a.join("fonts").join("NotoSansSC-Regular.otf")))
        .or_else(|| CJK_FONTS.iter().find_map(|p| read_font(Path::new(p))))
}

/// Sets the window's fonts and look, once.
pub fn apply(ctx: &egui::Context) {
    use egui::epaint::text::{FontTweak, VariationCoords};
    let mut fonts = egui::FontDefinitions::default();
    let tektur = egui::FontData::from_static(TEKTUR).tweak(FontTweak {
        coords: VariationCoords::new([(b"wght", WEIGHT)]),
        ..Default::default()
    });
    fonts.font_data.insert("tektur".into(), Arc::new(tektur));
    fonts.font_data.insert("noto".into(), Arc::new(egui::FontData::from_static(NOTO)));
    let chinese = chinese_font();
    let has_chinese = chinese.is_some();
    if let Some(bytes) = chinese {
        fonts.font_data.insert("chinese".into(), Arc::new(egui::FontData::from_owned(bytes)));
    }
    // each family: its face, then Chinese, then egui's own fonts (symbols)
    let rest = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let chain = |first: &[&str]| -> Vec<String> {
        first
            .iter()
            .map(|s| (*s).to_owned())
            .chain(has_chinese.then(|| "chinese".to_owned()))
            .chain(rest.iter().cloned())
            .collect()
    };
    fonts.families.insert(FontFamily::Name(CAPS_FAMILY.into()), chain(&["tektur", "noto"]));
    fonts.families.insert(FontFamily::Proportional, chain(&["noto"]));
    if has_chinese {
        fonts.families.entry(FontFamily::Monospace).or_default().push("chinese".into());
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    use egui::TextStyle as S;
    style.text_styles = [
        (S::Heading, FontId::new(20.0, FontFamily::Name(CAPS_FAMILY.into()))),
        (S::Body, FontId::new(14.0, FontFamily::Proportional)),
        (S::Button, FontId::new(14.0, FontFamily::Proportional)),
        (S::Small, FontId::new(12.0, FontFamily::Proportional)),
        (S::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.slider_width = 170.0;
    style.spacing.slider_rail_height = 6.0;
    style.spacing.combo_width = 120.0;

    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgb(34, 32, 44);
    v.window_stroke = Stroke::new(1.5, METAL_DIM);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = rgba(8, 7, 12, 200);
    v.text_edit_bg_color = Some(rgba(8, 7, 12, 200));
    v.faint_bg_color = rgba(255, 255, 255, 10);
    v.selection.bg_fill = rgba(244, 170, 80, 150);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.warn_fg_color = GOLD;
    v.error_fg_color = ERROR;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    let pill = CornerRadius::same(14);
    let w = &mut v.widgets;
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.bg_stroke = Stroke::new(1.0, rgba(133, 122, 149, 120));
    w.noninteractive.corner_radius = pill;
    w.inactive.bg_fill = rgba(70, 68, 82, 230);
    w.inactive.weak_bg_fill = rgba(58, 57, 68, 220);
    w.inactive.bg_stroke = Stroke::new(1.0, METAL_DIM);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = pill;
    for state in [&mut w.hovered, &mut w.active, &mut w.open] {
        state.bg_fill = Color32::from_rgb(120, 66, 60);
        state.weak_bg_fill = Color32::from_rgb(104, 58, 56);
        state.bg_stroke = Stroke::new(1.5, GOLD);
        state.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        state.corner_radius = pill;
        state.expansion = 0.0;
    }
    style.visuals = v;
    ctx.set_global_style(style);
}

/// A text in the interface's face: lower case as small capitals, spaced, as the game draws its labels.
pub fn caps(text: &str, size: f32, color: Color32, wrap: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let format = |small: bool| egui::TextFormat {
        font_id: FontId::new(if small { size * SMALL_CAPS } else { size }, FontFamily::Name(CAPS_FAMILY.into())),
        extra_letter_spacing: size * TRACKING,
        color,
        valign: egui::Align::BOTTOM,
        ..Default::default()
    };
    // runs of one size: a lower-case letter with one capital is that capital, smaller
    let mut run = String::new();
    let mut run_small = false;
    for c in text.chars() {
        let mut up = c.to_uppercase();
        let (shown, small) = match (up.next(), up.next()) {
            (Some(u), None) if c.is_lowercase() && u != c => (u, true),
            _ => (c, false),
        };
        // spaces and marks stay in the run they are in
        let small = if c.is_alphanumeric() { small } else { run_small };
        if small != run_small && !run.is_empty() {
            job.append(&run, 0.0, format(run_small));
            run.clear();
        }
        run_small = small;
        run.push(shown);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, format(run_small));
    }
    job
}

/// A heading in the interface's face with the game's dark shadow under it, wrapped to the width there is.
pub fn heading(ui: &mut Ui, text: &str, size: f32, color: Color32, centred: bool) -> Response {
    let wrap = ui.available_width();
    let mut job = caps(text, size, color, wrap);
    let mut shadow = caps(text, size, rgba(0, 0, 0, 170), wrap);
    if centred {
        job.halign = egui::Align::Center;
        shadow.halign = egui::Align::Center;
    }
    let galley = ui.painter().layout_job(job);
    let shadow = ui.painter().layout_job(shadow);
    let size = egui::vec2(if centred { wrap } else { galley.size().x }, galley.size().y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let at = if centred { rect.center_top() } else { rect.left_top() };
        ui.painter().galley(at + egui::vec2(1.0, 1.5), shadow, Color32::BLACK);
        ui.painter().galley(at, galley, color);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text));
    response
}

/// The sky behind everything and the dark band down the middle, `band` wide.
pub fn backdrop(ui: &Ui, band: f32) {
    let rect = ui.max_rect();
    let mut mesh = egui::Mesh::default();
    let rows = [(rect.top(), SKY_TOP), (rect.center().y, SKY_MID), (rect.bottom(), SKY_BOTTOM)];
    for (y, color) in rows {
        mesh.colored_vertex(Pos2::new(rect.left(), y), color);
        mesh.colored_vertex(Pos2::new(rect.right(), y), color);
    }
    for i in [0u32, 2] {
        mesh.add_triangle(i, i.saturating_add(1), i.saturating_add(2));
        mesh.add_triangle(i.saturating_add(1), i.saturating_add(2), i.saturating_add(3));
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let band = Rect::from_center_size(rect.center(), egui::vec2(band.min(rect.width()), rect.height()));
    ui.painter().rect_filled(band, 0.0, rgba(24, 22, 34, 150));
}

/// A framed dark panel, as the game's hint panel: what a section of the window sits in.
pub fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::new()
        .fill(rgba(16, 14, 20, 205))
        .stroke(Stroke::new(1.5, METAL_DIM))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 14));
    let shown = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    // the frame's dark inner line, as on the game's metal frames
    let inner = shown.response.rect.shrink(3.0);
    ui.painter().rect_stroke(inner, CornerRadius::same(9), Stroke::new(1.0, rgba(0, 0, 0, 120)), StrokeKind::Inside);
    shown.inner
}

/// The ball of a button: silver, or gold on the chosen one.
fn ball(painter: &egui::Painter, centre: Pos2, radius: f32, gold: bool, alpha: f32) {
    let (dark, base, light) = if gold {
        ([120, 70, 20], [226, 150, 44], [255, 242, 166])
    } else {
        ([70, 70, 80], [168, 168, 180], [254, 255, 255])
    };
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| {
        let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
        let [r, g, bl] = [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])];
        rgba(r, g, bl, (255.0 * alpha) as u8)
    };
    painter.circle_filled(centre, radius + 1.0, mix([30, 29, 36], [30, 29, 36], 0.0));
    painter.circle_filled(centre, radius, mix(dark, dark, 0.0));
    // the light falls from the upper left: smaller, lighter discs towards it
    const STEPS: u8 = 7;
    for i in 0..STEPS {
        let t = f32::from(i) / f32::from(STEPS - 1);
        let colour = if t < 0.6 { mix(dark, base, 0.35 + t / 0.6 * 0.65) } else { mix(base, light, (t - 0.6) / 0.4) };
        let shift = radius * 0.36 * t;
        painter.circle_filled(centre - egui::vec2(shift, shift), radius * (0.96 - 0.80 * t), colour);
    }
}

/// The body of a pill button: its fill, its metal rim (gold when chosen) and the rim's dark edges.
fn pill_body(painter: &egui::Painter, rect: Rect, chosen: bool, rim: f32, alpha: f32) {
    let fade = |c: Color32| c.gamma_multiply(alpha);
    let round = CornerRadius::same((rect.height() / 2.0).round().clamp(0.0, 255.0) as u8);
    let fill = if chosen { rgba(124, 66, 60, 235) } else { rgba(62, 61, 72, 225) };
    painter.rect_filled(rect, round, fade(EDGE));
    painter.rect_filled(rect.shrink(1.0), round, fade(if chosen { GOLD } else { METAL }));
    painter.rect_filled(rect.shrink(1.0 + rim), round, fade(EDGE));
    painter.rect_filled(rect.shrink(2.0 + rim), round, fade(fill));
}

/// A small pill button with a label in the interface's face.
pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let enabled = enabled && ui.is_enabled();
    let colour = if enabled { TEXT } else { TEXT_WEAK.gamma_multiply(0.6) };
    let galley = ui.painter().layout_job(caps(text, 13.0, colour, f32::INFINITY));
    let size = egui::vec2(galley.size().x + 30.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let chosen = enabled && (response.hovered() || response.has_focus());
        pill_body(ui.painter(), rect, chosen, 1.5, if enabled { 1.0 } else { 0.5 });
        let at = Align2::CENTER_CENTER.align_size_within_rect(galley.size(), rect).min;
        ui.painter().galley(at, galley, colour);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

/// For looking at the chosen state without a cursor (debug builds only): the main buttons are drawn chosen.
#[cfg(debug_assertions)]
pub static SHOW_CHOSEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn shown_chosen() -> bool {
    #[cfg(debug_assertions)]
    return SHOW_CHOSEN.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(debug_assertions))]
    false
}

/// The main button of a section, as a button of the game's menu: a wide pill with the ball at its left end, gold
/// under the cursor; centred in the width there is.
pub fn main_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    const HEIGHT: f32 = 44.0;
    let enabled = enabled && ui.is_enabled();
    let colour = if enabled { Color32::WHITE } else { TEXT_WEAK.gamma_multiply(0.6) };
    let avail = ui.available_width();
    let galley = ui.painter().layout_job(caps(text, 17.0, colour, f32::INFINITY));
    let shadow = ui.painter().layout_job(caps(text, 17.0, rgba(0, 0, 0, 170), f32::INFINITY));
    let width = (galley.size().x + 2.0 * HEIGHT + 24.0).max(300.0).min(avail);
    let (row, _) = ui.allocate_exact_size(egui::vec2(avail, HEIGHT), Sense::hover());
    let rect = Rect::from_center_size(row.center(), egui::vec2(width, HEIGHT));
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let response = ui.interact(rect, ui.next_auto_id().with(text), sense);
    if ui.is_rect_visible(rect) {
        let chosen = enabled && (response.hovered() || response.has_focus() || shown_chosen());
        let alpha = if enabled { 1.0 } else { 0.5 };
        pill_body(ui.painter(), rect, chosen, 2.5, alpha);
        let centre = Pos2::new(rect.left() + HEIGHT / 2.0, rect.center().y);
        // the rim's arc round the ball, as on the game's buttons
        let arc = if chosen { GOLD } else { METAL };
        ui.painter().circle_stroke(centre, HEIGHT / 2.0 - 7.0, Stroke::new(1.5, arc.gamma_multiply(alpha * 0.8)));
        ball(ui.painter(), centre, HEIGHT / 2.0 - 11.0, chosen, alpha);
        let at = Align2::CENTER_CENTER.align_size_within_rect(galley.size(), rect).min;
        if enabled {
            ui.painter().galley(at + egui::vec2(1.0, 1.5), shadow, Color32::BLACK);
        }
        ui.painter().galley(at, galley, colour);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

/// A line that opens and closes what lies under it: a small triangle and a label in the interface's face.
pub fn fold(ui: &mut Ui, text: &str, open: &mut bool) -> Response {
    let galley = ui.painter().layout_job(caps(text, 13.0, TEXT_WEAK, f32::INFINITY));
    let size = egui::vec2(galley.size().x + 22.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *open = !*open;
    }
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        let colour = if lit { GOLD } else { TEXT_WEAK };
        let c = Pos2::new(rect.left() + 7.0, rect.center().y);
        let points = if *open {
            vec![c + egui::vec2(-5.0, -3.0), c + egui::vec2(5.0, -3.0), c + egui::vec2(0.0, 4.0)]
        } else {
            vec![c + egui::vec2(-3.0, -5.0), c + egui::vec2(4.0, 0.0), c + egui::vec2(-3.0, 5.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
        let at = Pos2::new(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, colour);
        if lit {
            let recolour = ui.painter().layout_job(caps(text, 13.0, GOLD, f32::INFINITY));
            ui.painter().galley(at, recolour, GOLD);
        }
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The run's progress: a pill filling with gold, its count in the middle.
pub fn progress(ui: &mut Ui, part: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let round = CornerRadius::same(11);
    let painter = ui.painter();
    painter.rect_filled(rect, round, EDGE);
    painter.rect_filled(rect.shrink(1.0), round, METAL_DIM);
    painter.rect_filled(rect.shrink(2.0), round, Color32::from_rgb(20, 18, 26));
    let inside = rect.shrink(3.0);
    let width = (inside.width() * part.clamp(0.0, 1.0)).max(inside.height());
    let filled = Rect::from_min_size(inside.min, egui::vec2(width, inside.height()));
    painter.rect_filled(filled, round, Color32::from_rgb(214, 140, 48));
    let top = Rect::from_min_size(filled.min, egui::vec2(filled.width(), filled.height() / 2.0)).shrink2(Vec2::new(5.0, 1.0));
    painter.rect_filled(top, round, rgba(255, 236, 170, 70));
    let font = FontId::new(12.0, FontFamily::Name(CAPS_FAMILY.into()));
    painter.text(rect.center() + egui::vec2(1.0, 1.0), Align2::CENTER_CENTER, text, font.clone(), rgba(0, 0, 0, 200));
    painter.text(rect.center(), Align2::CENTER_CENTER, text, font, Color32::WHITE);
}

/// A thin metal line between the parts of a panel.
pub fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().hline(rect.x_range(), y, Stroke::new(1.0, rgba(133, 122, 149, 110)));
}

/// A picture in a metal frame.
pub fn framed_image(ui: &mut Ui, texture: &egui::TextureHandle, side: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), Sense::hover());
    if ui.is_rect_visible(rect) {
        let inside = rect.shrink(3.0);
        ui.painter().rect_filled(rect, CornerRadius::same(4), EDGE);
        ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(3), METAL_DIM);
        egui::Image::new(texture).paint_at(ui, inside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_case_becomes_small_capitals() {
        let job = caps("Make it 4×", 10.0, TEXT, f32::INFINITY);
        assert_eq!(job.text, "MAKE IT 4×");
        // "M" at full size, "ake it " small, "4×" at full size
        let sizes: Vec<f32> = job.sections.iter().map(|s| s.format.font_id.size).collect();
        assert_eq!(sizes, vec![10.0, 8.0, 10.0]);
        // a letter without a single capital (ß) and Chinese stay as they are
        assert_eq!(caps("ß 关卡", 10.0, TEXT, f32::INFINITY).text, "ß 关卡");
    }

    #[test]
    fn the_built_in_fonts_are_fonts() {
        assert!(TEKTUR.starts_with(&[0, 1, 0, 0]));
        assert!(NOTO.starts_with(&[0, 1, 0, 0]));
    }
}
