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

/// Where the window keeps its last choices: the game's data folder (the remastered levels' home too).
fn choices_file() -> PathBuf {
    gamedata::profile::data_dir().join("toolbox.txt")
}

/// The model and scale chosen last (`model NAME` and `scale N` lines); none when there is no such file or line.
fn remembered() -> (Option<String>, Option<u32>) {
    use std::io::Read;
    let mut text = String::new();
    let plain = std::fs::symlink_metadata(choices_file()).is_ok_and(|m| m.is_file());
    if plain && let Ok(f) = std::fs::File::open(choices_file()) {
        let _ = f.take(4096).read_to_string(&mut text);
    }
    let value = |key: &str| {
        text.lines()
            .filter_map(|l| l.split_once(' '))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.trim().to_owned())
    };
    let scale = value("scale").and_then(|v| v.parse::<u32>().ok()).filter(|n| (1..=8).contains(n));
    (value("model").filter(|m| !m.is_empty() && m.len() <= 64), scale)
}

/// The folder the file chooser was last left in (`dir PATH` line), when it is still a folder.
fn remembered_dir() -> Option<PathBuf> {
    use std::io::Read;
    let mut text = String::new();
    let plain = std::fs::symlink_metadata(choices_file()).is_ok_and(|m| m.is_file());
    if plain && let Ok(f) = std::fs::File::open(choices_file()) {
        let _ = f.take(8192).read_to_string(&mut text);
    }
    let dir = text.lines().find_map(|l| l.strip_prefix("dir "))?;
    let dir = PathBuf::from(dir.trim());
    dir.is_dir().then_some(dir)
}

/// Keeps the model, scale and chooser folder for next time (written beside and moved in; a failure only loses the choice).
fn remember(model: &str, scale: u32, dir: Option<&Path>) {
    let dir_line = dir
        .filter(|d| !d.to_string_lossy().contains(['\n', '\r']))
        .map_or(String::new(), |d| format!("dir {}\n", d.display()));
    let path = choices_file();
    let mut tmp = path.clone().into_os_string();
    tmp.push(".new");
    let tmp = PathBuf::from(tmp);
    let writable = |p: &Path| std::fs::symlink_metadata(p).map_or(true, |m| m.is_file());
    if writable(&path) && writable(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        if std::fs::write(&tmp, format!("model {model}\nscale {scale}\n{dir_line}")).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// What a file chooser was opened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    /// The game copy as a folder.
    CopyFolder,
    /// The game copy as a CD image file (.cue, .bin, .iso).
    CopyImage,
    /// The folder of downloaded community maps.
    Maps,
}

/// Where the chooser starts: the folder the field names (or the folder holding the file it names), else the folder
/// chosen last; none when neither is there.
fn start_dir(field: &str, last: Option<&Path>) -> Option<PathBuf> {
    let typed = PathBuf::from(field.trim());
    if !typed.as_os_str().is_empty() {
        if typed.is_dir() {
            return Some(typed);
        }
        if let Some(parent) = typed.parent().filter(|p| p.is_dir()) {
            return Some(parent.to_path_buf());
        }
    }
    last.filter(|d| d.is_dir()).map(Path::to_path_buf)
}

/// The folder to remember after a pick: the folder itself, or the one holding the picked file.
fn dir_of_pick(picked: &Path, folder: bool) -> Option<PathBuf> {
    if folder { Some(picked.to_path_buf()) } else { picked.parent().map(Path::to_path_buf) }
}

/// Opens the system's chooser on its own thread (the window keeps drawing); the answer comes through the channel,
/// none when it was cancelled. A backend that is not there (no portal) ends the thread without an answer.
fn open_chooser(kind: Pick, title: &str, start: Option<PathBuf>, ctx: &egui::Context) -> Receiver<Option<PathBuf>> {
    let (send, answer) = channel();
    let (title, ctx) = (title.to_owned(), ctx.clone());
    std::thread::spawn(move || {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if let Some(d) = start {
            dialog = dialog.set_directory(d);
        }
        let picked = match kind {
            Pick::CopyFolder | Pick::Maps => dialog.pick_folder(),
            Pick::CopyImage => dialog
                .add_filter("CD image", &["cue", "bin", "iso", "img"])
                .add_filter("all files", &["*"])
                .pick_file(),
        };
        let _ = send.send(picked);
        ctx.request_repaint();
    });
    answer
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
    /// The folder of downloaded community maps to import.
    maps_field: String,
    job: Option<Job>,
    /// A file chooser open: what for, and its answer.
    chooser: Option<(Pick, Receiver<Option<PathBuf>>)>,
    last_dir: Option<PathBuf>,
    step: Option<(usize, usize, String)>,
    lines: Vec<String>,
    outcome: Option<Result<(), String>>,
    preview: Option<(egui::TextureHandle, egui::TextureHandle)>,
    /// The last run was an import of community maps (its result says where to play them).
    imported: bool,
    last_texture: Option<(PathBuf, PathBuf)>,
}

impl App {
    fn new(data_dir: Option<PathBuf>) -> App {
        let data = gamedata::assets::Data::locate(data_dir)
            .ok()
            .and_then(|d| d.root);
        let models = remaster::models();
        // the model and the scale chosen last time, when they are still there
        let (kept_model, kept_scale) = remembered();
        let model = match kept_model.filter(|m| models.contains(m)) {
            Some(m) => m,
            None if models.iter().any(|m| m == "realesrgan-x4plus") => "realesrgan-x4plus".to_owned(),
            None => models.first().cloned().unwrap_or_default(),
        };
        App {
            lang: texts::system(),
            data_field: data
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
            data,
            scale: kept_scale.unwrap_or(4),
            models,
            model,
            relief: 0.0,
            maps_field: String::new(),
            job: None,
            chooser: None,
            last_dir: remembered_dir(),
            step: None,
            lines: Vec::new(),
            outcome: None,
            preview: None,
            imported: false,
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
        let last_dir = self.last_dir.clone();
        remember(&model, scale, last_dir.as_deref());
        std::thread::spawn(move || {
            let data = gamedata::assets::Data::new(Some(root));
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
                .and_then(|()| remaster::run_in(&data, &out, scale, &model, relief, &[], &mut watch))
                .and_then(|mut lines| {
                    let sounds = gamedata::profile::data_dir().join("sounds");
                    lines.push(remaster::original_sounds(&data, &sounds)?);
                    Ok(lines)
                });
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

    /// Starts importing the community maps of the folder typed (import.rs) on a worker thread.
    fn start_import(&mut self, ctx: &egui::Context) {
        let from = PathBuf::from(self.maps_field.trim());
        if from.as_os_str().is_empty() {
            return;
        }
        let (send, reports) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (stop, ctx2) = (cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let to = gamedata::profile::dir(gamedata::profile::Kind::Downloaded);
            let (tell, repaint) = (send.clone(), ctx2.clone());
            let mut line = move |l: String| {
                let _ = tell.send(Ok(Progress::Line(l)));
                repaint.request_repaint();
            };
            let sorting = crate::import::Sorting { catalogue: None, unsorted: crate::import::default_unsorted(&to) };
            let result = crate::import::import(&from, &to, &sorting, &mut line, &stop);
            if let Ok(s) = &result {
                let _ = send.send(Ok(Progress::Line(crate::import::summary_line(s, &to))));
                // what the game does not support yet in each map, so the player knows before playing
                if s.imported > 0 {
                    crate::import::check_maps(&to, &mut |l| {
                        let _ = send.send(Ok(Progress::Line(l)));
                        ctx2.request_repaint();
                    });
                }
            }
            let _ = send.send(result.map(|_| Progress::Step { done: 0, total: 0, what: String::new() }));
            ctx2.request_repaint();
        });
        self.job = Some(Job { reports, cancel });
        self.lines.clear();
        self.step = None;
        self.outcome = None;
        self.imported = true;
    }

    /// Opens the chooser for a field.
    fn browse(&mut self, kind: Pick, ctx: &egui::Context) {
        if self.chooser.is_some() {
            return;
        }
        let l = self.lang;
        let (field, title) = match kind {
            Pick::CopyFolder => (&self.data_field, texts::PICK_COPY.get(l)),
            Pick::CopyImage => (&self.data_field, texts::PICK_IMAGE.get(l)),
            Pick::Maps => (&self.maps_field, texts::PICK_MAPS.get(l)),
        };
        let start = start_dir(field, self.last_dir.as_deref());
        self.chooser = Some((kind, open_chooser(kind, title, start, ctx)));
    }

    /// Takes the chooser's answer: the path fills its field and is checked as a typed one is.
    fn poll_chooser(&mut self) {
        let Some((kind, answer)) = &self.chooser else { return };
        let kind = *kind;
        let picked = match answer.try_recv() {
            Ok(p) => p,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                eprintln!("the system's file chooser is not available; type the path instead");
                None
            }
        };
        self.chooser = None;
        let Some(path) = picked else { return };
        if let Some(d) = dir_of_pick(&path, kind != Pick::CopyImage) {
            self.last_dir = Some(d);
            remember(&self.model, self.scale, self.last_dir.as_deref());
        }
        match kind {
            Pick::CopyFolder | Pick::CopyImage => {
                self.data_field = path.display().to_string();
                self.apply_copy();
            }
            Pick::Maps => self.maps_field = path.display().to_string(),
        }
    }

    /// Checks the path in the copy field: the folder itself, never another copy found instead of it.
    fn apply_copy(&mut self) {
        let typed = PathBuf::from(self.data_field.trim());
        self.data = gamedata::assets::Data::locate(Some(typed.clone()))
            .ok()
            .and_then(|d| d.root)
            .filter(|root| *root == typed);
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
                    // unless its last word came in this very round: that one stands
                    finished.get_or_insert(Err("the work stopped unexpectedly".to_owned()));
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
        self.poll_chooser();
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
                            egui::TextEdit::singleline(&mut self.data_field).desired_width(300.0),
                        );
                        if ui
                            .add_enabled(!busy, egui::Button::new(texts::USE.get(l)))
                            .clicked()
                        {
                            self.apply_copy();
                        }
                        let free = !busy && self.chooser.is_none();
                        if ui.add_enabled(free, egui::Button::new(texts::BROWSE.get(l))).clicked() {
                            self.browse(Pick::CopyFolder, ctx);
                        }
                        if ui.add_enabled(free, egui::Button::new(texts::IMAGE.get(l))).clicked() {
                            self.browse(Pick::CopyImage, ctx);
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
                    self.imported = false;
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
                    ui.label(if self.imported { texts::PLAY_MAPS.get(l) } else { texts::PLAY.get(l) });
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

            // community maps: a downloaded collection sorted into the game's level folders
            ui.label(texts::MAPS.get(l));
            ui.horizontal(|ui| {
                ui.add_enabled(!busy, egui::TextEdit::singleline(&mut self.maps_field).desired_width(300.0));
                if ui.add_enabled(!busy && self.chooser.is_none(), egui::Button::new(texts::BROWSE.get(l))).clicked() {
                    self.browse(Pick::Maps, ctx);
                }
                let ready = !busy && !self.maps_field.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new(texts::IMPORT.get(l))).clicked() {
                    self.start_import(ctx);
                }
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooser_starts_where_the_field_points() {
        let tmp = std::env::temp_dir();
        let t = tmp.display().to_string();
        assert_eq!(start_dir(&t, None), Some(tmp.clone()));
        // a file path that is not there: the folder holding it
        assert_eq!(start_dir(&format!("{t}/no-such-image.cue"), None), Some(tmp.clone()));
        // nothing typed: the folder chosen last, if it is still a folder
        assert_eq!(start_dir("  ", Some(&tmp)), Some(tmp.clone()));
        assert_eq!(start_dir("", Some(Path::new("/no/such/dir/anywhere"))), None);
        assert_eq!(start_dir("", None), None);
    }

    #[test]
    fn remembered_folder_is_the_pick_or_its_parent() {
        assert_eq!(dir_of_pick(Path::new("/a/b"), true), Some(PathBuf::from("/a/b")));
        assert_eq!(dir_of_pick(Path::new("/a/b/game.cue"), false), Some(PathBuf::from("/a/b")));
    }
}
