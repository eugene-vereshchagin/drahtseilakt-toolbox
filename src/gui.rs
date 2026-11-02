// The toolbox's window (egui on wgpu, as the game draws): where the player's copy is, how the levels are to be
// remastered, a Make button, and the run's progress, result and a texture before and after. The run goes on a worker
// thread and reports through a channel, so the window stays responsive, and Cancel stops it between textures. Texts
// come from texts.rs in the system's language, switchable; a font for Chinese is taken from the system (never
// downloaded), else the window keeps egui's own fonts (Latin, Cyrillic, Greek).

use crate::remaster::{self, Progress, Watch};
use crate::texts::{self, LANGS, Lang};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};

/// The largest font file read, and the largest preview side.
const FONT_MAX: u64 = 64 * 1024 * 1024;
const PREVIEW_SIDE: u32 = 256;

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

/// Opens the window; returns when it is closed.
pub fn run(data_dir: Option<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 720.0])
            .with_min_inner_size([560.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "drahtseilakt-toolbox",
        options,
        Box::new(move |cc| {
            add_system_font(&cc.egui_ctx);
            Ok(Box::new(App::new(data_dir)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Adds a system font with Chinese after egui's own, for the texts they lack.
fn add_system_font(ctx: &egui::Context) {
    let Some(bytes) = CJK_FONTS.iter().find_map(|p| read_font(Path::new(p))) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "system-cjk".into(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("system-cjk".into());
    }
    ctx.set_fonts(fonts);
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

/// A run going on: its reports and the flag that stops it.
struct Job {
    reports: Receiver<Result<Progress, String>>,
    cancel: Arc<AtomicBool>,
}

struct App {
    lang: Lang,
    data: Option<PathBuf>,
    data_field: String,
    scale: u32,
    models: Vec<String>,
    model: String,
    relief: f32,
    job: Option<Job>,
    step: Option<(usize, usize, String)>,
    lines: Vec<String>,
    outcome: Option<Result<(), String>>,
    preview: Option<(egui::TextureHandle, egui::TextureHandle)>,
    last_texture: Option<(PathBuf, PathBuf)>,
}

impl App {
    fn new(data_dir: Option<PathBuf>) -> App {
        let data = gamedata::assets::Data::locate(data_dir)
            .ok()
            .map(|d| d.root);
        let models = remaster::models();
        let model = if models.iter().any(|m| m == "realesrgan-x4plus") {
            "realesrgan-x4plus".to_owned()
        } else {
            models.first().cloned().unwrap_or_default()
        };
        App {
            lang: texts::system(),
            data_field: data
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
            data,
            scale: 4,
            models,
            model,
            relief: 0.0,
            job: None,
            step: None,
            lines: Vec::new(),
            outcome: None,
            preview: None,
            last_texture: None,
        }
    }

    /// Starts the run on a worker thread.
    fn start(&mut self, ctx: &egui::Context) {
        let Some(root) = self.data.clone() else {
            return;
        };
        let (send, reports) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (stop, ctx2) = (cancel.clone(), ctx.clone());
        let (scale, model, relief) = (self.scale, self.model.clone(), self.relief);
        std::thread::spawn(move || {
            let data = gamedata::assets::Data { root, overlay: None };
            let out = gamedata::profile::dir(gamedata::profile::Kind::Remastered);
            let tell = send.clone();
            let repaint = ctx2.clone();
            let mut report = move |p: Progress| {
                let _ = tell.send(Ok(p));
                repaint.request_repaint();
            };
            let mut watch = Watch {
                report: &mut report,
                cancel: &stop,
            };
            let result = std::fs::create_dir_all(&out)
                .map_err(|e| format!("{}: {e}", out.display()))
                .and_then(|()| remaster::run_in(&data, &out, scale, &model, relief, &mut watch));
            let _ = send.send(result.map(|_| Progress::Step {
                done: 0,
                total: 0,
                what: String::new(),
            }));
            ctx2.request_repaint();
        });
        self.job = Some(Job { reports, cancel });
        self.lines.clear();
        self.step = None;
        self.outcome = None;
    }

    /// Takes what the run has reported since the last frame.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let mut finished = None;
        loop {
            let r = match job.reports.try_recv() {
                Ok(r) => r,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // the worker is gone without its last word (it failed): the run is over
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    finished = Some(Err("the work stopped unexpectedly".to_owned()));
                    break;
                }
            };
            match r {
                Ok(Progress::Step { done, total, what }) if total > 0 => {
                    self.step = Some((done, total, what))
                }
                // the worker's last word: the run is over
                Ok(Progress::Step { .. }) => finished = Some(Ok(())),
                Ok(Progress::Line(l)) => self.lines.push(l),
                Ok(Progress::Texture { original, grown }) => {
                    self.last_texture = Some((original, grown))
                }
                Err(e) => finished = Some(Err(e)),
            }
        }
        if let Some((original, grown)) = self.last_texture.take() {
            // a preview at most a few times a second is enough; the newest texture wins
            if let (Some(a), Some(b)) =
                (thumb(ctx, &original, "before"), thumb(ctx, &grown, "after"))
            {
                self.preview = Some((a, b));
            }
        }
        if let Some(f) = finished {
            self.outcome = Some(f);
            self.job = None;
        }
    }
}

/// A texture file as a small image for the window.
fn thumb(ctx: &egui::Context, path: &Path, name: &str) -> Option<egui::TextureHandle> {
    let img = remaster::load(path).ok()?;
    let img = image::imageops::thumbnail(
        &img,
        PREVIEW_SIDE.min(img.width()),
        PREVIEW_SIDE.min(img.height()),
    );
    let size = [
        usize::try_from(img.width()).ok()?,
        usize::try_from(img.height()).ok()?,
    ];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::NEAREST))
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        self.poll(ctx);
        let l = self.lang;
        let busy = self.job.is_some();
        egui::CentralPanel::default().show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading(texts::TITLE.get(l));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("lang")
                        .selected_text(LANGS.iter().find(|x| x.0 == l).map_or("", |x| x.1))
                        .show_ui(ui, |ui| {
                            for (lang, name) in LANGS {
                                ui.selectable_value(&mut self.lang, lang, name);
                            }
                        });
                    ui.label(texts::LANGUAGE.get(l));
                });
            });
            ui.label(texts::INTRO.get(l));
            ui.separator();

            egui::Grid::new("settings")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label(texts::COPY.get(l));
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::singleline(&mut self.data_field).desired_width(420.0),
                        );
                        if ui
                            .add_enabled(!busy, egui::Button::new(texts::USE.get(l)))
                            .clicked()
                        {
                            // the typed folder itself, never another copy found instead of it
                            let typed = PathBuf::from(self.data_field.trim());
                            self.data = gamedata::assets::Data::locate(Some(typed.clone()))
                                .ok()
                                .map(|d| d.root)
                                .filter(|root| *root == typed);
                        }
                    });
                    ui.end_row();
                    if self.data.is_none() {
                        ui.label("");
                        ui.colored_label(egui::Color32::LIGHT_RED, texts::NO_COPY.get(l));
                        ui.end_row();
                    }

                    ui.label(texts::OUTPUT.get(l));
                    ui.label(
                        gamedata::profile::dir(gamedata::profile::Kind::Remastered)
                            .display()
                            .to_string(),
                    );
                    ui.end_row();

                    ui.label(texts::SCALE.get(l));
                    ui.add_enabled(!busy, egui::Slider::new(&mut self.scale, 1..=8).suffix("×"));
                    ui.end_row();

                    ui.label(texts::MODEL.get(l));
                    if self.models.is_empty() {
                        ui.label(texts::LANCZOS.get(l));
                    } else {
                        ui.add_enabled_ui(!busy, |ui| {
                            egui::ComboBox::from_id_salt("model")
                                .selected_text(self.model.as_str())
                                .show_ui(ui, |ui| {
                                    for m in &self.models {
                                        ui.selectable_value(&mut self.model, m.clone(), m.as_str());
                                    }
                                });
                        });
                    }
                    ui.end_row();

                    ui.label(texts::RELIEF.get(l));
                    ui.add_enabled(
                        !busy,
                        egui::Slider::new(&mut self.relief, 0.0..=8.0).step_by(0.5),
                    );
                    ui.end_row();
                });
            if self.relief > 0.0 {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 170, 60),
                    texts::RELIEF_WARN.get(l),
                );
            }
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if busy {
                    let cancelling = self
                        .job
                        .as_ref()
                        .is_some_and(|j| j.cancel.load(std::sync::atomic::Ordering::Relaxed));
                    if cancelling {
                        ui.label(texts::CANCELLING.get(l));
                    } else if ui.button(texts::CANCEL.get(l)).clicked()
                        && let Some(j) = &self.job
                    {
                        j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    ui.spinner();
                } else if ui
                    .add_enabled(self.data.is_some(), egui::Button::new(texts::MAKE.get(l)))
                    .clicked()
                {
                    self.start(ctx);
                }
            });
            if let Some((done, total, what)) = &self.step {
                let part = if *total > 0 {
                    *done as f32 / *total as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(part).text(format!("{done} / {total}")));
                if busy {
                    ui.label(format!("{} {what}", texts::WORKING.get(l)));
                }
            }
            match &self.outcome {
                Some(Ok(())) => {
                    ui.label(texts::DONE.get(l));
                    ui.label(texts::PLAY.get(l));
                }
                Some(Err(e)) => {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        format!("{} {e}", texts::FAILED.get(l)),
                    );
                }
                None => {}
            }
            ui.separator();

            ui.columns(2, |cols| {
                if let Some(ui) = cols.get_mut(0) {
                    ui.label(texts::RESULT.get(l));
                    egui::ScrollArea::vertical()
                        .id_salt("lines")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.lines {
                                ui.label(egui::RichText::new(line).small());
                            }
                        });
                }
                if let (Some(ui), Some((before, after))) = (cols.get_mut(1), &self.preview) {
                    ui.label(texts::PREVIEW.get(l));
                    ui.horizontal(|ui| {
                        let side = egui::vec2(160.0, 160.0);
                        ui.add(egui::Image::new(before).fit_to_exact_size(side));
                        ui.add(egui::Image::new(after).fit_to_exact_size(side));
                    });
                }
            });
        });
    }
}
