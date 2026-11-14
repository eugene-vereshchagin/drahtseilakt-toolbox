// The toolbox's window (egui on wgpu, as the game draws), in the look of the game's own menu (theme.rs): two sections,
// "Remaster the original levels" (where the player's copy is, how much larger the textures get, the technical
// settings folded under Advanced, a Make button) and "Import community maps" (the folder, an Import button), then
// the run's progress, result and a texture before and after. The run goes on a worker thread and reports through a
// channel, so the window stays responsive, and Cancel stops it between textures. Texts come from texts.rs in the
// system's language, switchable.

use crate::remaster::{self, Progress, Watch};
use crate::texts::{self, LANGS, Lang};
use crate::theme;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};

/// The largest preview side.
const PREVIEW_SIDE: u32 = 256;
/// The widest the window's column of sections gets, and its margin to the window's edge.
const COLUMN_MAX: f32 = 660.0;
const COLUMN_MARGIN: f32 = 28.0;

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
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(App::new(data_dir)))
        }),
    )
    .map_err(|e| e.to_string())
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
    /// The technical settings (the model, the relief) are shown.
    advanced: bool,
    /// The result is to be scrolled into view (a run has started or ended).
    reveal: bool,
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
            advanced: false,
            reveal: false,
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
        self.reveal = true;
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
        self.reveal = true;
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
            self.reveal = true;
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

/// A path field with its buttons: on one line when the panel is wide, the buttons under the field when it is
/// narrow. Returns whether Enter was pressed in the field, and the button clicked.
fn path_row(ui: &mut egui::Ui, field: &mut String, enabled: bool, buttons: &[(&str, bool)]) -> (bool, Option<usize>) {
    let mut clicked = None;
    let mut entered = false;
    let mut edit = |ui: &mut egui::Ui, width: f32| {
        let text = egui::TextEdit::singleline(field)
            .desired_width(width)
            .margin(egui::Margin::symmetric(10, 5));
        let r = ui.add_enabled(enabled, text);
        entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    };
    if buttons.len() > 1 && ui.available_width() < 540.0 {
        edit(ui, ui.available_width());
        ui.horizontal(|ui| {
            for (k, (text, on)) in buttons.iter().enumerate() {
                if theme::button(ui, text, *on).clicked() {
                    clicked = Some(k);
                }
            }
        });
    } else {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (k, (text, on)) in buttons.iter().enumerate().rev() {
                    if theme::button(ui, text, *on).clicked() {
                        clicked = Some(k);
                    }
                }
                edit(ui, ui.available_width());
            });
        });
    }
    (entered, clicked)
}

/// A small label over a field: what the field is.
fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(theme::TEXT_WEAK));
}

impl App {
    /// The first section: the player's copy, the scale, the technical settings, Make (Cancel while it runs).
    fn remaster_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let l = self.lang;
        let busy = self.job.is_some();
        theme::heading(ui, texts::REMASTER_TITLE.get(l), 17.0, theme::GOLD, false);
        caption(ui, texts::INTRO.get(l));
        ui.add_space(2.0);
        ui.label(texts::COPY.get(l));
        let free = !busy && self.chooser.is_none();
        let buttons = [
            (texts::USE.get(l), !busy),
            (texts::BROWSE.get(l), free),
            (texts::IMAGE.get(l), free),
        ];
        let (entered, clicked) = path_row(ui, &mut self.data_field, !busy, &buttons);
        match clicked {
            Some(0) => self.apply_copy(),
            Some(1) => self.browse(Pick::CopyFolder, ctx),
            Some(2) => self.browse(Pick::CopyImage, ctx),
            _ if entered => self.apply_copy(),
            _ => {}
        }
        if self.data.is_none() {
            ui.colored_label(theme::ERROR, texts::NO_COPY.get(l));
        }
        ui.horizontal_wrapped(|ui| {
            caption(ui, texts::OUTPUT.get(l));
            let out = gamedata::profile::dir(gamedata::profile::Kind::Remastered);
            caption(ui, &out.display().to_string());
        });
        theme::rule(ui);

        ui.horizontal_wrapped(|ui| {
            ui.label(texts::SCALE.get(l));
            ui.add_enabled(!busy, egui::Slider::new(&mut self.scale, 1..=8).suffix("×"));
        });
        theme::fold(ui, texts::ADVANCED.get(l), &mut self.advanced);
        if self.advanced {
            ui.horizontal_wrapped(|ui| {
                ui.label(texts::MODEL.get(l));
                if self.models.is_empty() {
                    caption(ui, texts::LANCZOS.get(l));
                } else {
                    ui.add_enabled_ui(!busy, |ui| {
                        egui::ComboBox::from_id_salt("model")
                            .selected_text(self.model.as_str())
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for m in &self.models {
                                    ui.selectable_value(&mut self.model, m.clone(), m.as_str());
                                }
                            });
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(texts::RELIEF.get(l));
                ui.add_enabled(!busy, egui::Slider::new(&mut self.relief, 0.0..=8.0).step_by(0.5));
            });
            if self.relief > 0.0 {
                ui.colored_label(theme::GOLD, texts::RELIEF_WARN.get(l));
            }
        }
        ui.add_space(4.0);
        if busy && !self.imported {
            self.cancel_button(ui);
        } else if theme::main_button(ui, texts::MAKE.get(l), !busy && self.data.is_some()).clicked() {
            self.imported = false;
            self.start(ctx);
        }
    }

    /// The second section: the folder of downloaded maps and Import (Cancel while it runs).
    fn maps_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let l = self.lang;
        let busy = self.job.is_some();
        theme::heading(ui, texts::MAPS_TITLE.get(l), 17.0, theme::GOLD, false);
        caption(ui, texts::MAPS.get(l));
        ui.add_space(2.0);
        let buttons = [(texts::BROWSE.get(l), !busy && self.chooser.is_none())];
        let (_, clicked) = path_row(ui, &mut self.maps_field, !busy, &buttons);
        if clicked == Some(0) {
            self.browse(Pick::Maps, ctx);
        }
        ui.add_space(4.0);
        let ready = !busy && !self.maps_field.trim().is_empty();
        if busy && self.imported {
            self.cancel_button(ui);
        } else if theme::main_button(ui, texts::IMPORT.get(l), ready).clicked() {
            self.start_import(ctx);
        }
    }

    /// Cancel in the place of the running section's button; once pressed, what the run still finishes.
    fn cancel_button(&mut self, ui: &mut egui::Ui) {
        let l = self.lang;
        let Some(job) = &self.job else { return };
        if job.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            ui.vertical_centered(|ui| ui.label(texts::CANCELLING.get(l)));
        } else if theme::main_button(ui, texts::CANCEL.get(l), true).clicked() {
            job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The third section, once there is something to show: the progress, the outcome, the run's lines and the last
    /// texture before and after.
    fn result_section(&mut self, ui: &mut egui::Ui) {
        let l = self.lang;
        let busy = self.job.is_some();
        theme::heading(ui, texts::RESULT.get(l), 17.0, theme::GOLD, false);
        if let Some((done, total, what)) = &self.step {
            let part = if *total > 0 { *done as f32 / *total as f32 } else { 0.0 };
            theme::progress(ui, part, &format!("{done} / {total}"));
            if busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0).color(theme::GOLD));
                    ui.add(egui::Label::new(format!("{} {what}", texts::WORKING.get(l))).wrap());
                });
            }
        } else if busy {
            ui.add(egui::Spinner::new().size(14.0).color(theme::GOLD));
        }
        match &self.outcome {
            Some(Ok(())) => {
                ui.label(egui::RichText::new(texts::DONE.get(l)).strong().color(theme::GOLD));
                ui.label(if self.imported { texts::PLAY_MAPS.get(l) } else { texts::PLAY.get(l) });
            }
            Some(Err(e)) => {
                ui.colored_label(theme::ERROR, format!("{} {e}", texts::FAILED.get(l)));
            }
            None => {}
        }
        if !self.lines.is_empty() {
            theme::rule(ui);
            egui::ScrollArea::vertical()
                .id_salt("lines")
                .max_height(150.0)
                .auto_shrink([false, true])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.lines {
                        ui.label(egui::RichText::new(line).small().color(theme::TEXT_WEAK));
                    }
                });
        }
        if let Some((before, after)) = &self.preview {
            theme::rule(ui);
            caption(ui, texts::PREVIEW.get(l));
            ui.horizontal_wrapped(|ui| {
                let side = ((ui.available_width() - 12.0) / 2.0).clamp(96.0, 200.0);
                theme::framed_image(ui, before, side);
                theme::framed_image(ui, after, side);
            });
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        #[cfg(debug_assertions)]
        self.demo(ctx);
        self.poll(ctx);
        self.poll_chooser();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let column = (ui.available_width() - 2.0 * COLUMN_MARGIN).clamp(200.0, COLUMN_MAX);
            theme::backdrop(ui, column + 28.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let side = ((ui.available_width() - column) / 2.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width(column);
                        ui.add_space(10.0);
                        let l = self.lang;
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
                        theme::heading(ui, texts::TITLE.get(l), 24.0, egui::Color32::WHITE, true);
                        ui.add_space(6.0);
                        theme::panel(ui, |ui| self.remaster_section(ui, ctx));
                        ui.add_space(4.0);
                        theme::panel(ui, |ui| self.maps_section(ui, ctx));
                        let shown = self.job.is_some()
                            || self.step.is_some()
                            || self.outcome.is_some()
                            || !self.lines.is_empty()
                            || self.preview.is_some();
                        if shown {
                            ui.add_space(4.0);
                            let result = ui.scope(|ui| theme::panel(ui, |ui| self.result_section(ui)));
                            if std::mem::take(&mut self.reveal) {
                                result.response.scroll_to_me(Some(egui::Align::BOTTOM));
                            }
                        }
                        ui.add_space(16.0);
                    });
                });
            });
        });
    }
}

/// A made-up state of the window, for looking at it without a run (debug builds only): `TOOLBOX_DEMO` holds any of
/// `busy`, `done`, `failed`, `nocopy`, `advanced`, `narrow`, `chosen`, split by commas.
#[cfg(debug_assertions)]
impl App {
    fn demo(&mut self, ctx: &egui::Context) {
        static ONCE: std::sync::Once = std::sync::Once::new();
        let mut first = false;
        ONCE.call_once(|| first = true);
        let Some(words) = std::env::var("TOOLBOX_DEMO").ok().filter(|_| first) else { return };
        let has = |w: &str| words.split(',').any(|x| x == w);
        if has("nocopy") {
            self.data = None;
            self.data_field.clear();
        }
        if has("advanced") {
            self.advanced = true;
            self.relief = 2.0;
        }
        if has("chosen") {
            theme::SHOW_CHOSEN.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if has("narrow") {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(560.0, 480.0)));
        }
        if has("busy") || has("done") || has("failed") {
            self.reveal = true;
            self.step = Some((37, 120, "Level_03: Floor_Top_Profil.bmp".to_owned()));
            self.lines = (1..=9).map(|n| format!("Level_{n:02}: 214 meshes rounded, 38 textures 4× larger")).collect();
            self.maps_field = "/home/player/Downloads/maps".to_owned();
            let tile = |a: [u8; 3], b: [u8; 3], cell: usize, name: &str| {
                let side = 96usize;
                let mut rgba = Vec::new();
                for y in 0..side {
                    for x in 0..side {
                        let c = if (x / cell + y / cell).is_multiple_of(2) { a } else { b };
                        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied([side, side], &rgba);
                ctx.load_texture(name, image, egui::TextureOptions::NEAREST)
            };
            self.preview = Some((
                tile([150, 120, 90], [120, 96, 70], 24, "before"),
                tile([160, 126, 92], [112, 90, 66], 12, "after"),
            ));
        }
        if has("busy") {
            let (send, reports) = channel();
            // the sender is kept for the program's life, so the made-up run never ends
            std::mem::forget(send);
            self.job = Some(Job { reports, cancel: Arc::new(AtomicBool::new(false)) });
        }
        if has("done") {
            self.step = Some((120, 120, String::new()));
            self.outcome = Some(Ok(()));
        }
        if has("failed") {
            self.outcome = Some(Err("levels/ballance-remaster: permission denied".to_owned()));
        }
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
