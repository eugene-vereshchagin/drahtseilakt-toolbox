// Remastered levels (docs/drahtseilakt/levels.md, roadmap phase 7): the original levels converted to the open format
// in levels/remastered, with rounder shapes (smooth.rs, written into the level as each mesh's look) and their textures
// made larger on the player's machine. A texture is upscaled by Real-ESRGAN
// when the player has installed it (`realesrgan-ncnn-vulkan` on the PATH: it adds detail), else by Lanczos with a
// light sharpening (larger and sharper, no new detail). The originals are kept beside (textures.original/), so a
// second run starts from them again. A colour-keyed texture gets its key as alpha first (its edges are blended when
// it grows, so the exact key colour would no longer match); the key stays in the level file, which keeps the alpha
// test on and lets the next run key the original again. The folders are the player's and may hold anything: links
// are never followed or written through, every file is written beside and moved into place, and a texture that
// cannot be read is left as it is and reported.

use gamedata::assets::{Data, Source};
use image::{RgbaImage, imageops};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// How far a run has got, for a window to show while it goes on.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// Working on step `done + 1` of `total` (a level or a shared part), named `what`.
    Step {
        done: usize,
        total: usize,
        what: String,
    },
    /// A texture was made larger: its original and the new one.
    Texture { original: PathBuf, grown: PathBuf },
    /// One line of the result (as the command line prints them at the end).
    Line(String),
}

/// What a run reports to and the flag that stops it (between textures, levels and parts).
pub struct Watch<'a> {
    pub report: &'a mut dyn FnMut(Progress),
    pub cancel: &'a AtomicBool,
}

impl Watch<'_> {
    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// The line a run cancelled ends with.
pub const CANCELLED: &str =
    "cancelled: the levels made before are whole, the rest left as they were";
/// What a level stopped among its textures is left as.
const PART_GROWN: &str =
    "stopped among its textures: some grown, the rest as they were (the next run grows them all)";

/// The largest side a texture is made (4K; the originals are 128 to 256).
const SIDE_MAX: u32 = 4096;
/// The smallest side of a texture Real-ESRGAN is given; smaller ones are resampled with Lanczos.
const ESRGAN_MIN_SIDE: u32 = 64;
/// The largest level file and texture read.
const GLTF_MAX: u64 = 64 * 1024 * 1024;
const IMAGE_MAX: u64 = 64 * 1024 * 1024;

/// Converts every original level the copy has that is not in levels/remastered yet, gives every level there its
/// rounder shapes, and upscales its textures by `factor` (1: left as they are). Returns a line a level, and one for
/// each texture left as it was.
pub fn run(
    data: &Data,
    factor: u32,
    model: &str,
    relief_strength: f32,
    only: &[String],
) -> Result<Vec<String>, String> {
    let cancel = AtomicBool::new(false);
    let mut watch = Watch {
        report: &mut |_| {},
        cancel: &cancel,
    };
    let root = gamedata::profile::dir(gamedata::profile::Kind::Remastered);
    let mut out = run_in(data, &root, factor, model, relief_strength, only, &mut watch)?;
    out.push(original_sounds(data, &gamedata::profile::data_dir().join("sounds"))?);
    Ok(out)
}

/// The largest sound file of the copy taken into the "original" theme.
const SOUND_MAX: u64 = 64 * 1024 * 1024;

/// Makes the "original" sound theme in `sounds` (the data folder's sounds/, which the game scans): the copy's own
/// Sounds/*.wav, music among them, with a theme.txt. It stays on the player's machine (it is the original's): the
/// game never ships it. Folders are made one by one, never through a link; files written beside and moved in.
pub fn original_sounds(data: &Data, sounds: &Path) -> Result<String, String> {
    let Some(from) = data.file("Sounds").filter(|p| real_dir(p)) else {
        return Ok("sounds: the copy has no Sounds folder; no original theme made".into());
    };
    for dir in [sounds.to_path_buf(), sounds.join("original"), sounds.join("original").join("Sounds")] {
        match std::fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("{}: not a folder", dir.display())),
            Err(_) => std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?,
        }
    }
    let theme = sounds.join("original");
    let mut names: Vec<String> = std::fs::read_dir(&from)
        .map_err(|e| format!("{}: {e}", from.display()))?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| {
            n.to_ascii_lowercase().ends_with(".wav")
                && !n.starts_with('.')
                && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
        })
        .take(1024)
        .collect();
    names.sort();
    let mut copied = 0usize;
    for n in &names {
        let src = from.join(n);
        let small = std::fs::symlink_metadata(&src).is_ok_and(|m| m.is_file() && m.len() <= SOUND_MAX);
        if !small {
            continue;
        }
        // the game asks for `<name>.wav`: the file keeps its name with the extension in lower case
        let stem = n.get(..n.len().saturating_sub(4)).unwrap_or(n);
        let to = theme.join("Sounds").join(format!("{stem}.wav"));
        if replace(&to, |t| std::fs::copy(&src, t).map(drop).map_err(|e| e.to_string())).is_ok() {
            copied += 1;
        }
    }
    let txt = "# The original's sounds and music, taken from the player's own copy by drahtseilakt-toolbox; never shipped.\n\
               name.en Original\nname.ru Оригинальная\nname.de Original\nname.zh 原版\n\
               author Cyparade / Atari (the player's own copy)\nlicence the player's own copy, not to be shared\n\
               fallback system\n";
    replace(&theme.join("theme.txt"), |t| std::fs::write(t, txt).map_err(|e| e.to_string()))?;
    Ok(format!("{}: {copied} of the copy's sounds", theme.display()))
}

/// [`run`] into the remastered folder `root`, reporting to `watch` and stopping when it is cancelled.
pub fn run_in(
    data: &Data,
    root: &Path,
    factor: u32,
    model: &str,
    relief_strength: f32,
    only: &[String],
    watch: &mut Watch<'_>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    if let Ok(mut m) = GROWN.lock() {
        m.clear();
    }
    if let Ok(mut m) = COMPRESSED.lock() {
        m.clear();
    }
    // `--levels`: only the level folders named (a number is an original level, `7` = Level_07); empty: all of them
    let wanted = |name: &str| {
        only.is_empty()
            || only.iter().any(|w| {
                w == name || w.parse::<u32>().is_ok_and(|n| format!("Level_{n:02}") == name)
            })
    };
    let line = |out: &mut Vec<String>, watch: &mut Watch<'_>, l: String| {
        (watch.report)(Progress::Line(l.clone()));
        out.push(l);
    };
    for n in 1..=data.level_count().unwrap_or(13) {
        let dir = root.join(format!("Level_{n:02}"));
        if !wanted(&format!("Level_{n:02}")) || std::fs::symlink_metadata(dir.join("level.gltf")).is_ok() {
            continue;
        }
        let Some(loaded) = from_copy(data, n) else {
            continue;
        };
        gamedata::assets::export(data, &loaded, &dir, true)?;
    }
    let mut levels: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| real_dir(p) && real_file(&p.join("level.gltf")))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(wanted))
        .collect();
    levels.sort();
    let esrgan = esrgan();
    if let Some(prog) = &esrgan {
        has_model(prog, model)?;
    }
    let esrgan = esrgan.as_deref().map(|p| (p, model));
    let total = levels.len();
    for (step, dir) in levels.into_iter().enumerate() {
        if watch.stopped() {
            line(&mut out, watch, CANCELLED.to_owned());
            return Ok(out);
        }
        let what = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (watch.report)(Progress::Step {
            done: step,
            total,
            what,
        });
        // an original level is made again from the player's copy each time (what the game reads of it may have
        // grown since: a remastered level is only ever derived from it); any other level from its own file
        let original = dir
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("Level_").map(|d| (n, d)))
            .and_then(|(n, d)| {
                d.parse::<u32>()
                    .ok()
                    .filter(|&k| format!("Level_{k:02}") == n)
            })
            .and_then(|n| from_copy(data, n));
        // its parts (balls, transformer, modules) and game files (sky, particles, shadow, sounds) go into its own
        // game folder, remade as it is, so it plays without the copy's level files
        let model = original.as_ref().map(|l| l.level.clone());
        match shapes(&dir, original.map(|l| l.level), Some(data)) {
            Ok(n) => line(
                &mut out,
                watch,
                format!("{}: {n} meshes rounder", dir.display()),
            ),
            Err(e) => line(
                &mut out,
                watch,
                format!("{}: shapes left as they were: {e}", dir.display()),
            ),
        }
        if factor >= 2 {
            match level(&dir, factor, esrgan, &mut out, watch) {
                Ok(n) => line(
                    &mut out,
                    watch,
                    format!(
                        "{}: {n} textures x{factor} ({})",
                        dir.display(),
                        if esrgan.is_some() {
                            "Real-ESRGAN"
                        } else {
                            "Lanczos"
                        }
                    ),
                ),
                Err(e) => line(
                    &mut out,
                    watch,
                    format!("{}: not remastered: {e}", dir.display()),
                ),
            }
        }
        // stopped among its textures: the level is left part grown (the next run grows it all again), no relief
        if watch.stopped() {
            line(&mut out, watch, format!("{}: {PART_GROWN}", dir.display()));
            line(&mut out, watch, CANCELLED.to_owned());
            return Ok(out);
        }
        match relief(&dir, factor, relief_strength) {
            Ok(n) => line(
                &mut out,
                watch,
                format!("{}: {n} normal maps", dir.display()),
            ),
            Err(e) => line(
                &mut out,
                watch,
                format!("{}: no relief: {e}", dir.display()),
            ),
        }
        match compress(&dir, watch) {
            Ok((n, bytes)) => line(
                &mut out,
                watch,
                format!("{}: {n} textures compressed for the graphics card ({} MB)", dir.display(), bytes / (1024 * 1024)),
            ),
            Err(e) => line(
                &mut out,
                watch,
                format!("{}: textures not compressed: {e}", dir.display()),
            ),
        }
        if let Some(model) = &model {
            let l = game_files(
                data,
                root,
                &dir,
                model,
                factor,
                esrgan,
                relief_strength,
                &mut out,
                watch,
            );
            line(&mut out, watch, l);
            if watch.stopped() {
                line(&mut out, watch, CANCELLED.to_owned());
                return Ok(out);
            }
        }
    }
    (watch.report)(Progress::Step {
        done: total,
        total,
        what: String::new(),
    });
    Ok(out)
}

/// `remaster-open`: the remaster of one open-format level folder in place (a shipped set's level, a downloaded one):
/// rounder shapes, textures `factor` times larger from `textures.original/` (kept there; a second run starts from them
/// again, so nothing is grown twice), masonry relief (and the brightness relief when `relief_strength` > 0) and the
/// compressed textures. Only looks and pictures change; the level's physics stays what it was. Returns the lines.
pub fn remaster_folder(dir: &Path, factor: u32, model: &str, relief_strength: f32) -> Result<Vec<String>, String> {
    if !real_dir(dir) || !real_file(&dir.join("level.gltf")) {
        return Err(format!("{}: not an open-format level folder", dir.display()));
    }
    let mut out = Vec::new();
    if let Ok(mut m) = GROWN.lock() {
        m.clear();
    }
    if let Ok(mut m) = COMPRESSED.lock() {
        m.clear();
    }
    let cancel = AtomicBool::new(false);
    let mut watch = Watch { report: &mut |_| {}, cancel: &cancel };
    let esrgan = esrgan();
    if let Some(prog) = &esrgan {
        has_model(prog, model)?;
    }
    let esrgan = esrgan.as_deref().map(|p| (p, model));
    let n = shapes(dir, None, None)?;
    out.push(format!("{}: {n} meshes rounder", dir.display()));
    if factor >= 2 {
        let n = level(dir, factor, esrgan, &mut out, &mut watch)?;
        out.push(format!(
            "{}: {n} textures x{factor} ({})",
            dir.display(),
            if esrgan.is_some() { "Real-ESRGAN" } else { "Lanczos" }
        ));
    }
    let n = relief(dir, factor, relief_strength)?;
    out.push(format!("{}: {n} normal maps", dir.display()));
    let (n, bytes) = compress(dir, &mut watch)?;
    out.push(format!("{}: {n} compressed textures ({} MB)", dir.display(), bytes / (1024 * 1024)));
    Ok(out)
}

/// `remaster-nmo`: the sidecar `<map>.remaster/` of a community map, made anew from the map each time (one left from
/// before is removed, links in it never followed), then remastered as an open-format folder. The map is only read; the
/// game files its scripts or parts need are not copied (the sidecar only gives looks).
pub fn remaster_nmo(file: &Path, factor: u32, model: &str, relief_strength: f32) -> Result<Vec<String>, String> {
    if !real_file(file) || file.extension().is_none_or(|e| !e.eq_ignore_ascii_case("nmo")) {
        return Err(format!("{}: not a .nmo file", file.display()));
    }
    // a map made for the original's textures, with no copy: the game's own textures by name, as the game draws it
    gamedata::packs::choose(gamedata::packs::Kind::Textures, gamedata::packs::OWN);
    let data = Data::new(None);
    let mut loaded = data.load(&Source::Nmo(file.to_path_buf()))?;
    // a texture the map names that the original's copy has and we have an own stand-in for: our own, by name, so the
    // sidecar carries no picture of the copy (it is shared); the others keep what they are
    for t in &mut loaded.level.textures {
        if let level::TextureSource::File(n) = &t.source
            && let Some(own) = gamedata::resources::own_texture(n)
            && let Ok(img) = load(&own)
        {
            t.source = level::TextureSource::Pixels {
                width: img.width(),
                height: img.height(),
                rgba: img.into_raw(),
            };
            t.color_key = None;
        }
    }
    let dir = file.with_extension("remaster");
    match std::fs::symlink_metadata(&dir) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?,
        Ok(_) => return Err(format!("{}: is not a folder", dir.display())),
        Err(_) => {}
    }
    gamedata::assets::export(&data, &loaded, &dir, false)?;
    remaster_folder(&dir, factor, model, relief_strength)
}

/// Writes a level's parts (the balls, the transformer, its modules) into its game folder, remade as the level is
/// (rounder shapes, larger textures, relief when asked), and copies the game files it uses (sky, particles, shadow,
/// sounds) beside them; returns the line that says how it went.
#[allow(clippy::too_many_arguments)]
fn game_files(
    data: &Data,
    root: &Path,
    dir: &Path,
    model: &level::Level,
    factor: u32,
    esrgan: Option<(&Path, &str)>,
    relief_strength: f32,
    out: &mut Vec<String>,
    watch: &mut Watch<'_>,
) -> String {
    let game = dir.join(gamedata::assets::GAME_FILES);
    let (mut parts, mut grown, mut files, mut compressed) = (0usize, 0usize, 0usize, 0usize);
    for rel in gamedata::assets::parts_used(data, model) {
        if watch.stopped() {
            break;
        }
        let Some(part) = data.part(&rel) else {
            continue;
        };
        let stem = rel
            .strip_suffix(".nmo")
            .or_else(|| rel.strip_suffix(".NMO"))
            .unwrap_or(&rel);
        let part_dir = game.join(stem);
        if let Err(e) = real_dirs(root, &part_dir.join("textures")) {
            out.push(format!("{}: not remade: {e}", part_dir.display()));
            continue;
        }
        if let Err(e) = shapes(&part_dir, Some(part.level), Some(data)) {
            out.push(format!("{}: not remade: {e}", part_dir.display()));
            continue;
        }
        parts += 1;
        if factor >= 2 {
            match level(&part_dir, factor, esrgan, out, watch) {
                Ok(n) => grown += n,
                Err(e) => out.push(format!(
                    "{}: textures left as they were: {e}",
                    part_dir.display()
                )),
            }
        }
        if let Err(e) = relief(&part_dir, factor, relief_strength) {
            out.push(format!("{}: no relief: {e}", part_dir.display()));
        }
        match compress(&part_dir, watch) {
            Ok((n, _)) => compressed += n,
            Err(e) => out.push(format!("{}: textures not compressed: {e}", part_dir.display())),
        }
    }
    if real_dirs(root, &game).is_ok() {
        for rel in gamedata::assets::files_used(data, model) {
            let Some(src) = data.file(&rel) else { continue };
            match gamedata::assets::copy_into(&src, &game, &rel) {
                Ok(()) => files += 1,
                Err(e) => out.push(format!("{rel}: not copied: {e}")),
            }
        }
    }
    format!(
        "{}: {parts} parts remade ({grown} of their textures x{factor}, {compressed} compressed), {files} game files",
        game.display()
    )
}

/// Original level `n` from the copy's own file; none when the copy lacks it (the game then plays the remastered
/// export in its place, which must never be made from itself).
fn from_copy(data: &Data, n: u32) -> Option<gamedata::assets::LoadedLevel> {
    matches!(data.original(n), Some(Source::Nmo(_)))
        .then(|| data.load(&Source::Original(n)).ok())
        .flatten()
}

/// Makes the folders from `root` down to `dir` one at a time, each a real folder (a link on the way is refused).
fn real_dirs(root: &Path, dir: &Path) -> Result<(), String> {
    let rest = dir
        .strip_prefix(root)
        .map_err(|_| "outside the remastered folder".to_owned())?;
    let mut at = root.to_path_buf();
    for part in rest.components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("{}: not a folder", at.display())),
            Err(_) => std::fs::create_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?,
        }
    }
    Ok(())
}

/// Gives a level's meshes their rounder looks, made again from the meshes themselves (so a better toolbox replaces
/// what an older one made), and writes the level file beside and moves it into place; `fresh` is the level made anew
/// from the player's copy, else the folder's own is read. A texture the level names that the folder lacks is written
/// from the copy. Returns how many meshes got a look.
fn shapes(dir: &Path, fresh: Option<level::Level>, data: Option<&Data>) -> Result<usize, String> {
    let mut level = match fresh {
        Some(l) => l,
        None => {
            let l = level::open::import(dir)?;
            // read back, its textures are files in the folder; written again they are named as an export names them
            for (i, t) in l.textures.iter().enumerate() {
                let ours = dir.join(level::open::texture_file(i, &t.name));
                let named = match &t.source {
                    level::TextureSource::File(p) => *p == ours.to_string_lossy(),
                    level::TextureSource::LevelFile(p) => *p == ours,
                    _ => false,
                };
                if !named {
                    return Err("its textures are not named as the game exports them".into());
                }
            }
            l
        }
    };
    let n = crate::smooth::looks(&mut level);
    write_level(dir, &level)?;
    let textures = dir.join("textures");
    for (i, t) in level.textures.iter().enumerate() {
        let path = dir.join(level::open::texture_file(i, &t.name));
        if std::fs::symlink_metadata(&path).is_ok() || !real_dir(&textures) {
            continue;
        }
        if let Some((w, h, rgba)) = data.and_then(|d| d.texture(t)) {
            replace(&path, |tmp| {
                image::save_buffer_with_format(
                    tmp,
                    &rgba,
                    w,
                    h,
                    image::ExtendedColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())
            })?;
        }
    }
    Ok(n)
}

/// Writes the level file (level.gltf and level.bin) of `dir` beside and moves it into place.
fn write_level(dir: &Path, level: &level::Level) -> Result<(), String> {
    // a fresh folder of our own to write into (one left from before is removed, links in it never followed)
    let tmp = dir.join(".remaster-level");
    match std::fs::symlink_metadata(&tmp) {
        Ok(m) if m.is_dir() => {
            std::fs::remove_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?
        }
        Ok(_) => return Err(".remaster-level is not a folder".into()),
        Err(_) => {}
    }
    std::fs::create_dir(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    level::open::export(level, &tmp)?;
    let names = ["level.bin", "level.gltf"];
    for name in names {
        let (from, to) = (tmp.join(name), dir.join(name));
        if !real_file(&from) || !writable(&to) {
            return Err(format!("{}: not a plain file", to.display()));
        }
    }
    // both places were checked just above; the buffer moves first and the level file last. Should the system fail
    // between the two, the level no longer loads (its accessors do not fit the new buffer, or it is refused); its
    // folder is then removed and the toolbox run again, which exports it anew from the player's copy.
    for name in names {
        std::fs::rename(tmp.join(name), dir.join(name))
            .map_err(|e| format!("{}: {e}", dir.join(name).display()))?;
    }
    let _ = std::fs::remove_dir(&tmp);
    Ok(())
}

/// Gives the level's lit, opaque or alpha-tested materials a normal map made from their texture as it is now (after
/// the upscale), written into textures/ beside the others and named in the level file. A level that has normal maps
/// already (one not made from the copy anew) is left as it is. Returns how many maps were made.
/// One remastered level's folder: its relief made again, its normal maps removed from the level first.
pub fn relief_folder(dir: &Path, scale: u32, strength: f32) -> Result<usize, String> {
    // a remastered level's own folder only (the toolbox made it: its originals kept beside), never a link to one
    if !real_dir(dir) || !real_dir(&dir.join("textures.original")) || !real_file(&dir.join("level.gltf")) {
        return Err(format!("{}: not a remastered level's folder", dir.display()));
    }
    let mut level = level::open::import(dir)?;
    // the textures that were only normal maps go, with their files, so that a second run does not add another set
    let used_as_normal: std::collections::HashSet<usize> = level.materials.iter().filter_map(|m| m.normal).collect();
    let used_as_texture: std::collections::HashSet<usize> =
        level.materials.iter().filter_map(|m| m.texture).collect();
    // texture files are named by their index: only the normal maps at the end (where the relief puts them) can go
    // without renaming the others; any before them are left as they are
    let n = level.textures.len();
    let tail_from = (0..n).rev().take_while(|i| used_as_normal.contains(i) && !used_as_texture.contains(i)).last();
    let mut map = Vec::with_capacity(n);
    let mut kept = Vec::new();
    for (i, t) in std::mem::take(&mut level.textures).into_iter().enumerate() {
        if tail_from.is_some_and(|f| i >= f) {
            let file = dir.join(level::open::texture_file(i, &t.name));
            // the file itself or a link put there: removed, never followed
            if std::fs::symlink_metadata(&file).is_ok_and(|m| !m.is_dir()) {
                let _ = std::fs::remove_file(&file);
            }
            map.push(None);
        } else {
            map.push(Some(kept.len()));
            kept.push(t);
        }
    }
    level.textures = kept;
    for m in &mut level.materials {
        m.normal = None;
        m.parallax = 0.0;
        m.texture = m.texture.and_then(|t| map.get(t).copied().flatten());
    }
    write_level(dir, &level)?;
    let n = relief(dir, scale, strength)?;
    // the normal maps are new pictures: their compressed textures are made again
    let cancel = AtomicBool::new(false);
    compress(
        dir,
        &mut Watch {
            report: &mut |_| {},
            cancel: &cancel,
        },
    )?;
    Ok(n)
}

fn relief(dir: &Path, scale: u32, strength: f32) -> Result<usize, String> {
    let mut level = level::open::import(dir)?;
    if level.materials.iter().any(|m| m.normal.is_some()) {
        return Ok(0);
    }
    if !real_dir(&dir.join("textures")) {
        return Err("textures/ missing or a link".into());
    }
    // each texture's map once: its index and whether it is masonry (drawn with parallax)
    let mut made: std::collections::HashMap<usize, (usize, bool)> = std::collections::HashMap::new();
    for k in 0..level.materials.len() {
        let Some(m) = level.materials.get(k) else {
            continue;
        };
        let Some(t) = m
            .texture
            .filter(|_| !m.unlit && m.texgen.is_none() && m.blend.is_none())
        else {
            continue;
        };
        let (normal, masonry) = match made.get(&t) {
            Some(&n) => n,
            None => {
                let Some(base) = level.textures.get(t) else {
                    continue;
                };
                let file = dir.join(level::open::texture_file(t, &base.name));
                let Ok(img) = load(&file) else { continue };
                // a tiny texture (a reflection map, a gradient) has no relief to give, as it has no detail to grow
                let original = file
                    .file_name()
                    .map(|n| dir.join("textures.original").join(n))
                    .and_then(|o| load(&o).ok());
                let (ow, oh) = original
                    .as_ref()
                    .map_or(img.dimensions(), |o| o.dimensions());
                if ow.min(oh) < ESRGAN_MIN_SIDE {
                    continue;
                }
                // masonry (bricks told by colour) always; the brightness relief only when asked for
                let masonry = original.as_ref().is_some_and(crate::masonry::is_masonry);
                let map = match (&original, masonry) {
                    (Some(o), true) => crate::masonry::map(o, img.dimensions()),
                    _ if strength > 0.0 => {
                        let grown = (img.width() / ow.max(1)).max(1).min(scale.max(1));
                        crate::normals::normal_map(&img, grown, strength)
                    }
                    _ => continue,
                };
                let name = format!("{}_normal", base.name);
                let n = level.textures.len();
                let path = dir.join(level::open::texture_file(n, &name));
                replace(&path, |tmp| {
                    map.save_with_format(tmp, image::ImageFormat::Png)
                        .map_err(|e| e.to_string())
                })?;
                level.textures.push(level::Texture {
                    name,
                    source: level::TextureSource::File(path.to_string_lossy().into_owned()),
                    color_key: None,
                });
                made.insert(t, (n, masonry));
                (n, masonry)
            }
        };
        if let Some(m) = level.materials.get_mut(k) {
            m.normal = Some(normal);
            m.parallax = if masonry { crate::masonry::DEPTH } else { 0.0 };
        }
    }
    if !made.is_empty() {
        write_level(dir, &level)?;
    }
    Ok(made.len())
}

/// A directory itself, not a link to one.
fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

/// A regular file itself, not a link to one.
fn real_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
}

/// Nothing there, or a regular file: a place a file may be written to (never a link, never a directory).
fn writable(p: &Path) -> bool {
    match std::fs::symlink_metadata(p) {
        Ok(m) => m.is_file(),
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

/// Writes `path` by writing its neighbour and moving it over (a link there is replaced, not written through; an
/// interruption leaves the old file).
fn replace(path: &Path, write: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    if !writable(path) {
        return Err(format!("{}: not a plain file", path.display()));
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".new");
    let tmp = PathBuf::from(tmp);
    if !writable(&tmp) {
        return Err(format!("{}: not a plain file", tmp.display()));
    }
    write(&tmp)?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A texture's file name as the level file gives it: a plain name under textures/ (no separator, drive colon, leading
/// dot or Windows device name).
fn texture_name(uri: &str) -> Option<&str> {
    let n = uri.strip_prefix("textures/")?;
    let stem = n.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4);
    let plain = !n.is_empty()
        && !n.starts_with('.')
        && !n
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':') || c.is_control());
    (plain && !device).then_some(n)
}

/// Real-ESRGAN's program: the one that comes with the toolbox (`realesrgan/` beside its own program, as its download
/// carries it), else one the player has on the PATH.
fn esrgan() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "realesrgan-ncnn-vulkan.exe"
    } else {
        "realesrgan-ncnn-vulkan"
    };
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("realesrgan").join(name)));
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(name));
    beside.into_iter().chain(on_path).find(|f| f.is_file())
}

/// The Real-ESRGAN models there are (each with its `.param` and `.bin` beside the program), by name; none when
/// Real-ESRGAN is not there.
pub fn models() -> Vec<String> {
    let Some(prog) = esrgan() else {
        return Vec::new();
    };
    let Some(dir) = prog.parent().map(|d| d.join("models")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".param")
                .map(str::to_owned)
        })
        .filter(|n| has_model(&prog, n).is_ok())
        .take(256)
        .collect();
    names.sort();
    names
}

/// The model is one of Real-ESRGAN's: a plain name with its `.param` and `.bin` in `models/` beside the program.
fn has_model(prog: &Path, model: &str) -> Result<(), String> {
    let plain = !model.is_empty()
        && model.len() <= 64
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !model.starts_with('.');
    let models = prog.parent().map(|d| d.join("models"));
    let there = models.is_some_and(|m| {
        ["param", "bin"]
            .iter()
            .all(|e| real_file(&m.join(format!("{model}.{e}"))))
    });
    if plain && there {
        Ok(())
    } else {
        Err(format!(
            "Real-ESRGAN has no model {model:?} in its models/ folder"
        ))
    }
}

/// Upscales one level's textures; returns how many. A texture that cannot be read or written is reported in `notes`
/// and left as it is.
fn level(
    dir: &Path,
    factor: u32,
    esrgan: Option<(&Path, &str)>,
    notes: &mut Vec<String>,
    watch: &mut Watch<'_>,
) -> Result<usize, String> {
    let gltf = dir.join("level.gltf");
    let small = real_file(&gltf) && std::fs::metadata(&gltf).is_ok_and(|m| m.len() <= GLTF_MAX);
    if !small {
        return Err("level.gltf missing, a link, or too big".into());
    }
    let text = std::fs::read_to_string(&gltf).map_err(|e| e.to_string())?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let (textures, originals) = (dir.join("textures"), dir.join("textures.original"));
    if !real_dir(&textures) {
        return Err("textures/ missing or a link".into());
    }
    match std::fs::symlink_metadata(&originals) {
        Ok(m) if !m.is_dir() => return Err("textures.original is not a folder".into()),
        Ok(_) => {}
        Err(_) => {
            std::fs::create_dir(&originals).map_err(|e| format!("{}: {e}", originals.display()))?
        }
    }
    let tmp = dir.join(".remaster-tmp");
    let mut done = 0;
    // the images the materials use as normal maps (made from the grown textures, never grown themselves)
    let normal_images: std::collections::HashSet<u64> = doc
        .get("materials")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m.pointer("/normalTexture/index").and_then(|i| i.as_u64()))
        .filter_map(|t| {
            let t = usize::try_from(t).ok()?;
            doc.get("textures")?
                .as_array()?
                .get(t)?
                .get("source")?
                .as_u64()
        })
        .collect();
    for (k, img) in doc
        .get("images")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
        .enumerate()
    {
        if watch.stopped() {
            break;
        }
        if u64::try_from(k).is_ok_and(|k| normal_images.contains(&k)) {
            continue;
        }
        let Some(name) = img
            .get("uri")
            .and_then(|u| u.as_str())
            .and_then(texture_name)
        else {
            continue;
        };
        let (path, original) = (textures.join(name), originals.join(name));
        let key = img
            .pointer("/extras/drahtseilakt/color_key")
            .and_then(key_of);
        let one = || -> Result<(), String> {
            // the original kept once, written beside and moved in, so an interruption never leaves half of it
            if !real_file(&original) {
                if !real_file(&path) {
                    return Err("missing or a link".into());
                }
                replace(&original, |t| {
                    std::fs::copy(&path, t).map(drop).map_err(|e| e.to_string())
                })?;
            }
            let source = load(&original)?;
            let source = match key {
                Some(k) => keyed(source, k),
                None => source,
            };
            // a tiny texture (a blurred reflection map, a gradient) has nothing for Real-ESRGAN to go on: it invents
            // detail there (cracks in a chrome reflection), so it is resampled
            let tiny = source.width().min(source.height()) < ESRGAN_MIN_SIDE;
            // the file keeps its own format (what the level file names)
            let format = image::ImageFormat::from_path(&path).map_err(|e| e.to_string())?;
            // the same picture grown before in this run (a level and its parts carry the same textures): that file
            // copied, so the work is done once and the copies are the same to the byte (the game then keeps one)
            let same = (
                gamedata::gputex::data_hash(source.as_raw()),
                source.dimensions(),
                factor,
                esrgan.map(|(_, m)| m.to_owned()),
                path.extension().map(|e| e.to_ascii_lowercase()),
            );
            let before = GROWN.lock().ok().and_then(|g| g.get(&same).cloned());
            if let Some(first) = before.filter(|f| *f != path && real_file(f)) {
                return replace(&path, |t| std::fs::copy(&first, t).map(drop).map_err(|e| e.to_string()));
            }
            let big = match esrgan.filter(|_| !tiny) {
                Some((prog, model)) => esrgan_upscale(prog, model, &source, factor, &tmp)
                    .unwrap_or_else(|_| lanczos(&source, factor)),
                None => lanczos(&source, factor),
            };
            replace(&path, |t| {
                let img = image::DynamicImage::ImageRgba8(big);
                let img = if format == image::ImageFormat::Jpeg {
                    image::DynamicImage::ImageRgb8(img.to_rgb8())
                } else {
                    img
                };
                img.save_with_format(t, format).map_err(|e| e.to_string())
            })?;
            if let Ok(mut g) = GROWN.lock() {
                g.insert(same, path.clone());
            }
            Ok(())
        };
        match one() {
            Ok(()) => {
                done += 1;
                (watch.report)(Progress::Texture {
                    original: original.clone(),
                    grown: path.clone(),
                });
            }
            Err(e) => {
                let l = format!("{}: left as it was: {e}", path.display());
                (watch.report)(Progress::Line(l.clone()));
                notes.push(l);
            }
        }
    }
    if real_dir(&tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Ok(done)
}

/// What makes a grown picture: its source's pixels (hash and size), the factor, the model, the file's format.
type GrownKey = (u64, (u32, u32), u32, Option<String>, Option<std::ffi::OsString>);
/// The pictures grown in this run, and the compressed textures made (by their picture file's length and hash).
static GROWN: std::sync::Mutex<std::collections::BTreeMap<GrownKey, PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());
static COMPRESSED: std::sync::Mutex<std::collections::BTreeMap<(u64, u64, bool), PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Writes the compressed texture (`name.dtx`, the game's gputex.rs) beside every large picture the level's materials
/// draw with: BC1 for an opaque colour picture, BC7 for one with alpha and for a normal map. The game loads it in
/// place of the picture (no decoding, an eighth or a quarter of the video memory); the picture stays, for a graphics
/// card without BC and for the next run. One made from the same picture before is left; a small picture's, or one
/// whose sides are not multiples of four, has none (one left from before is removed). Returns how many the level has
/// and their bytes.
fn compress(dir: &Path, watch: &mut Watch<'_>) -> Result<(usize, u64), String> {
    use gamedata::gputex;
    let level = level::open::import(dir)?;
    if !real_dir(&dir.join("textures")) {
        return Err("textures/ missing or a link".into());
    }
    let normals: std::collections::HashSet<usize> = level.materials.iter().filter_map(|m| m.normal).collect();
    let (mut count, mut bytes) = (0usize, 0u64);
    for (i, t) in level.textures.iter().enumerate() {
        if watch.stopped() {
            break;
        }
        let level::TextureSource::LevelFile(path) = &t.source else {
            continue;
        };
        // only the files the export names (plain ones in textures/)
        if *path != dir.join(level::open::texture_file(i, &t.name)) || !real_file(path) {
            continue;
        }
        let dtx = path.with_extension("dtx");
        let fine = normals.contains(&i);
        let one = || -> Result<Option<u64>, String> {
            let file = std::fs::read(path).map_err(|e| e.to_string())?;
            let source = (file.len() as u64, gputex::data_hash(&file));
            // made from this very picture already
            let made = |p: &Path| {
                use std::io::Read;
                let mut head = [0u8; gputex::HEADER_LEN];
                real_file(p)
                    && std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut head)).is_ok()
                    && gputex::header(&head).is_ok_and(|h| {
                        (h.source_len, h.source_hash) == source
                            && std::fs::metadata(p).is_ok_and(|m| m.len() == h.file_len())
                    })
            };
            if made(&dtx) {
                return Ok(std::fs::metadata(&dtx).ok().map(|m| m.len()));
            }
            // the same picture compressed before in this run: that file copied
            let same = (source.0, source.1, fine);
            let before = COMPRESSED.lock().ok().and_then(|c| c.get(&same).cloned());
            if let Some(first) = before.filter(|f| made(f)) {
                replace(&dtx, |tmp| std::fs::copy(&first, tmp).map(drop).map_err(|e| e.to_string()))?;
                return Ok(std::fs::metadata(&dtx).ok().map(|m| m.len()));
            }
            let img = load(path)?;
            let (w, h) = img.dimensions();
            if u64::from(w) * u64::from(h) < crate::bc::MIN_PIXELS || !w.is_multiple_of(4) || !h.is_multiple_of(4) {
                return Ok(None);
            }
            let format = crate::bc::format_for(img.as_raw(), fine);
            let levels = crate::bc::encode(w, h, img.as_raw(), format).ok_or("not whole blocks")?;
            let out = gputex::write(format, w, h, source, &levels)?;
            let len = out.len() as u64;
            replace(&dtx, |tmp| std::fs::write(tmp, &out).map_err(|e| e.to_string()))?;
            if let Ok(mut c) = COMPRESSED.lock() {
                c.insert(same, dtx.clone());
            }
            Ok(Some(len))
        };
        match one() {
            Ok(Some(len)) => {
                count += 1;
                bytes += len;
            }
            // no compressed texture for this picture: one left from before (of another picture) goes
            Ok(None) => {
                if std::fs::symlink_metadata(&dtx).is_ok_and(|m| !m.is_dir()) {
                    let _ = std::fs::remove_file(&dtx);
                }
            }
            Err(e) => {
                if std::fs::symlink_metadata(&dtx).is_ok_and(|m| !m.is_dir()) {
                    let _ = std::fs::remove_file(&dtx);
                }
                let l = format!("{}: not compressed: {e}", path.display());
                (watch.report)(Progress::Line(l));
            }
        }
    }
    Ok((count, bytes))
}

fn key_of(v: &serde_json::Value) -> Option<[u8; 3]> {
    let a = v.as_array()?;
    let c = |i: usize| {
        a.get(i)
            .and_then(|x| x.as_u64())
            .and_then(|x| u8::try_from(x).ok())
    };
    Some([c(0)?, c(1)?, c(2)?])
}

/// A texture, its size checked before it is decoded.
pub fn load(path: &Path) -> Result<RgbaImage, String> {
    let small = real_file(path) && std::fs::metadata(path).is_ok_and(|m| m.len() <= IMAGE_MAX);
    if !small {
        return Err(format!("{}: missing, a link or too big", path.display()));
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(SIDE_MAX);
    limits.max_image_height = Some(SIDE_MAX);
    reader.limits(limits);
    Ok(reader
        .decode()
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8())
}

/// The key colour made transparent, and the transparent texels given their opaque neighbours' colour, so that the
/// colour of the key never bleeds into the edges when the texture grows.
fn keyed(mut img: RgbaImage, key: [u8; 3]) -> RgbaImage {
    for p in img.pixels_mut() {
        if p.0[..3] == key {
            p.0[3] = 0;
        }
    }
    let (w, h) = img.dimensions();
    for _ in 0..8 {
        let src = img.clone();
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                if src.get_pixel(x, y).0[3] != 0 {
                    continue;
                }
                let (mut sum, mut n) = ([0u32; 3], 0u32);
                for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                    if nx < 0 || ny < 0 || nx >= i64::from(w) || ny >= i64::from(h) {
                        continue;
                    }
                    let q = src.get_pixel(nx as u32, ny as u32).0;
                    // an opaque texel, or one filled in an earlier pass
                    if q[3] != 0 || q[..3] != key {
                        for c in 0..3 {
                            sum[c] += u32::from(q[c]);
                        }
                        n += 1;
                    }
                }
                if n > 0 {
                    let p = img.get_pixel_mut(x, y);
                    for (out, s) in p.0.iter_mut().zip(sum) {
                        *out = s.checked_div(n).unwrap_or(0) as u8;
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    img
}

/// The target size: `factor` times, the longer side at most SIDE_MAX.
fn target(w: u32, h: u32, factor: u32) -> (u32, u32) {
    let f = factor.clamp(1, 32).min(SIDE_MAX / w.max(h).max(1)).max(1);
    (w * f, h * f)
}

/// Lanczos resampling and a light unsharp mask.
fn lanczos(img: &RgbaImage, factor: u32) -> RgbaImage {
    let (w, h) = target(img.width(), img.height(), factor);
    let f = w / img.width().max(1);
    let p = margin(img);
    let big = imageops::resize(
        &padded(img, p),
        w + 2 * p * f,
        h + 2 * p * f,
        imageops::FilterType::Lanczos3,
    );
    let big = imageops::crop_imm(&big, p * f, p * f, w, h).to_image();
    imageops::unsharpen(&big, 0.6 * (f as f32).sqrt(), 2)
}

/// The margin a texture gets around it while it grows: an eighth of its shorter side, at least 4 texels.
fn margin(img: &RgbaImage) -> u32 {
    (img.width().min(img.height()) / 8)
        .max(4)
        .min(img.width().min(img.height()))
}

/// The texture repeats as a tile: across its edges it changes no more than twice as much as within (a decal or a
/// picture does not, its edges do not meet).
pub fn tiles(img: &RgbaImage) -> bool {
    let (w, h) = img.dimensions();
    if w < 2 || h < 2 {
        return false;
    }
    let d = |a: &image::Rgba<u8>, b: &image::Rgba<u8>| -> u64 {
        a.0.iter()
            .zip(b.0)
            .map(|(x, y)| u64::from(x.abs_diff(y)))
            .sum()
    };
    let (mut across, mut within) = (0u64, 0u64);
    for y in 0..h {
        across += d(img.get_pixel(0, y), img.get_pixel(w - 1, y));
        within += d(img.get_pixel(0, y), img.get_pixel(1, y));
    }
    for x in 0..w {
        across += d(img.get_pixel(x, 0), img.get_pixel(x, h - 1));
        within += d(img.get_pixel(x, 0), img.get_pixel(x, 1));
    }
    across <= 2 * within.max(1)
}

/// The texture with a margin of `p` texels around it, so that what grows at its edges sees what is beyond them: the
/// texture's other side where it repeats as a tile (no seam where tiles meet), else its own edge mirrored.
fn padded(img: &RgbaImage, p: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let wrap = tiles(img);
    let at = |i: i64, n: u32| -> u32 {
        let n = i64::from(n);
        let j = if wrap {
            i.rem_euclid(n)
        } else {
            // mirrored: -1 -> 0, n -> n - 1
            let m = i.rem_euclid(2 * n);
            if m < n { m } else { 2 * n - 1 - m }
        };
        j as u32
    };
    let mut out = RgbaImage::new(w + 2 * p, h + 2 * p);
    for (x, y, px) in out.enumerate_pixels_mut() {
        *px = *img.get_pixel(
            at(i64::from(x) - i64::from(p), w),
            at(i64::from(y) - i64::from(p), h),
        );
    }
    out
}

/// Real-ESRGAN by 4 at a time (its model's scale), then Lanczos to the exact size; its files in `tmp`, a folder of
/// the level's own.
fn esrgan_upscale(
    prog: &Path,
    model: &str,
    img: &RgbaImage,
    factor: u32,
    tmp: &Path,
) -> Result<RgbaImage, String> {
    let (w, h) = target(img.width(), img.height(), factor);
    match std::fs::symlink_metadata(tmp) {
        Ok(m) if !m.is_dir() => {
            return Err("the remaster's temporary folder is not a folder".into());
        }
        Ok(_) => {}
        Err(_) => std::fs::create_dir(tmp).map_err(|e| e.to_string())?,
    }
    let (input, output) = (tmp.join("in.png"), tmp.join("out.png"));
    let mut cur = img.clone();
    while (cur.width() < w || cur.height() < h) && cur.width().max(cur.height()) * 4 <= SIDE_MAX {
        // each pass grows the texture with a margin around it (no seams where tiles meet), then cuts it off
        let p = margin(&cur);
        replace(&input, |t| {
            padded(&cur, p)
                .save_with_format(t, image::ImageFormat::Png)
                .map_err(|e| e.to_string())
        })?;
        if !writable(&output) {
            return Err("Real-ESRGAN's output is not a plain file".into());
        }
        let ok = std::process::Command::new(prog)
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .args(["-s", "4", "-n", model])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err("Real-ESRGAN failed".into());
        }
        let (cw, ch) = cur.dimensions();
        let grown = load(&output)?;
        if grown.dimensions() != ((cw + 2 * p) * 4, (ch + 2 * p) * 4) {
            return Err("Real-ESRGAN gave a picture of another size".into());
        }
        cur = imageops::crop_imm(&grown, p * 4, p * 4, cw * 4, ch * 4).to_image();
    }
    Ok(if cur.dimensions() == (w, h) {
        cur
    } else {
        imageops::resize(&cur, w, h, imageops::FilterType::Lanczos3)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_key_becomes_alpha_and_its_colour_does_not_bleed() {
        let mut img = RgbaImage::new(4, 1);
        for (x, c) in [[255, 0, 255], [10, 20, 30], [10, 20, 30], [255, 0, 255]]
            .into_iter()
            .enumerate()
        {
            img.put_pixel(x as u32, 0, image::Rgba([c[0], c[1], c[2], 255]));
        }
        let k = keyed(img, [255, 0, 255]);
        assert_eq!(k.get_pixel(0, 0).0, [10, 20, 30, 0]);
        assert_eq!(k.get_pixel(1, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn texture_names_are_plain() {
        assert_eq!(
            texture_name("textures/0001_Floor.png"),
            Some("0001_Floor.png")
        );
        for bad in [
            "textures/../x.png",
            "textures/a/b.png",
            "x.png",
            "textures/.hidden",
            "textures/CON.png",
            "textures/c:x",
        ] {
            assert_eq!(texture_name(bad), None, "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_left_over_folder_with_links_is_never_written_through() {
        let root = std::env::temp_dir().join(format!("drahtseilakt-shapes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("Level_01");
        let mut l = level::Level::default();
        l.meshes.push(level::Mesh {
            name: "t".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            parts: vec![level::Part {
                material: None,
                indices: vec![0, 1, 2],
            }],
            channels: Vec::new(),
            look: None,
        });
        level::open::export(&l, &dir).unwrap();
        // someone's file outside, and links to it where the toolbox writes
        let outside = root.join("outside.txt");
        std::fs::write(&outside, b"keep").unwrap();
        let tmp = dir.join(".remaster-level");
        std::fs::create_dir(&tmp).unwrap();
        std::os::unix::fs::symlink(&outside, tmp.join("level.gltf")).unwrap();
        std::os::unix::fs::symlink(&outside, tmp.join("level.bin")).unwrap();
        let data = Data::new(Some(root.clone()));
        assert_eq!(shapes(&dir, None, &data).unwrap(), 0);
        assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
        assert!(
            std::fs::symlink_metadata(dir.join("level.gltf"))
                .unwrap()
                .is_file()
        );
        assert!(level::open::import(&dir).is_ok());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_tile_is_padded_with_its_other_side_and_a_picture_with_its_mirror() {
        // a tile: columns 10, 20, 30, 20 (its right edge leads back into its left one)
        let mut tile = RgbaImage::new(4, 4);
        for (x, _, p) in tile.enumerate_pixels_mut() {
            let v = [10u8, 20, 30, 20][x as usize];
            *p = image::Rgba([v, v, v, 255]);
        }
        assert!(tiles(&tile));
        let t = padded(&tile, 2);
        assert_eq!(t.dimensions(), (8, 8));
        // left of column 0 come columns 3 and 2
        assert_eq!(t.get_pixel(0, 3).0[0], 30);
        assert_eq!(t.get_pixel(1, 3).0[0], 20);
        assert_eq!(t.get_pixel(2, 3).0[0], 10);
        // a picture whose edges do not meet: 0 at the left, 250 at the right
        let mut pic = RgbaImage::new(4, 4);
        for (x, _, p) in pic.enumerate_pixels_mut() {
            let v = [0u8, 80, 160, 250][x as usize];
            *p = image::Rgba([v, v, v, 255]);
        }
        assert!(!tiles(&pic));
        let m = padded(&pic, 2);
        assert_eq!(m.get_pixel(1, 0).0[0], 0, "its own edge mirrored");
        assert_eq!(m.get_pixel(0, 0).0[0], 80);
        // Lanczos keeps the tile's edges meeting: its left and right columns stay alike
        let big = lanczos(&tile, 4);
        assert_eq!(big.dimensions(), (16, 16));
    }

    /// A remastered folder with one level of one triangle and one 8x8 texture, outside any copy of the game.
    fn one_level(name: &str) -> (PathBuf, Data) {
        let root =
            std::env::temp_dir().join(format!("drahtseilakt-run-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("Mine");
        let mut l = level::Level::default();
        l.meshes.push(level::Mesh {
            name: "t".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            parts: vec![level::Part {
                material: None,
                indices: vec![0, 1, 2],
            }],
            channels: Vec::new(),
            look: None,
        });
        l.textures.push(level::Texture {
            name: "T".into(),
            source: level::TextureSource::File("T.png".into()),
            color_key: None,
        });
        level::open::export(&l, &dir).unwrap();
        std::fs::create_dir_all(dir.join("textures")).unwrap();
        let png = dir.join(level::open::texture_file(0, "T"));
        image::save_buffer(
            &png,
            &[200u8; 8 * 8 * 4],
            8,
            8,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        // an empty copy of the game: no originals (a level not made from the copy gets no parts)
        let copy = root.join("copy");
        std::fs::create_dir_all(&copy).unwrap();
        (
            root,
            Data::new(Some(copy)),
        )
    }

    #[test]
    fn a_run_reports_its_steps_and_textures_as_it_goes() {
        let (root, data) = one_level("progress");
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let mut report = |p: Progress| seen.push(p);
        let mut watch = Watch {
            report: &mut report,
            cancel: &cancel,
        };
        let out = run_in(&data, &root, 2, "realesrgan-x4plus", 0.0, &[], &mut watch).unwrap();
        assert!(
            seen.iter()
                .any(|p| matches!(p, Progress::Step { done: 0, total: 1, what } if what == "Mine")),
            "{seen:?}"
        );
        assert!(
            seen.iter().any(|p| matches!(p, Progress::Texture { .. })),
            "{seen:?}"
        );
        assert!(seen.iter().any(|p| matches!(
            p,
            Progress::Step {
                done: 1,
                total: 1,
                ..
            }
        )));
        // every result line was reported as it came, in the same order
        let lines: Vec<&String> = seen
            .iter()
            .filter_map(|p| {
                if let Progress::Line(l) = p {
                    Some(l)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(lines, out.iter().collect::<Vec<_>>());
        assert_eq!(
            load(&root.join("Mine").join(level::open::texture_file(0, "T")))
                .unwrap()
                .width(),
            16
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_cancelled_run_stops_and_says_so() {
        let (root, data) = one_level("cancel");
        let cancel = AtomicBool::new(true);
        let mut report = |_| {};
        let mut watch = Watch {
            report: &mut report,
            cancel: &cancel,
        };
        let out = run_in(&data, &root, 2, "realesrgan-x4plus", 0.0, &[], &mut watch).unwrap();
        assert_eq!(out, vec![CANCELLED.to_owned()]);
        // nothing was touched
        assert_eq!(
            load(&root.join("Mine").join(level::open::texture_file(0, "T")))
                .unwrap()
                .width(),
            8
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_original_sound_theme_is_made_from_the_copy() {
        let root = std::env::temp_dir().join(format!("drahtseilakt-osounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("copy/Sounds")).unwrap();
        std::fs::write(root.join("copy/Sounds/Misc_Fall.WAV"), b"RIFF").unwrap();
        std::fs::write(root.join("copy/Sounds/readme.txt"), b"x").unwrap();
        let data = Data::new(Some(root.join("copy")));
        let sounds = root.join("sounds");
        let line = original_sounds(&data, &sounds).unwrap();
        assert!(line.contains("1 of"), "{line}");
        assert!(sounds.join("original/Sounds/Misc_Fall.wav").is_file());
        assert!(!sounds.join("original/Sounds/readme.txt").exists());
        assert!(std::fs::read_to_string(sounds.join("original/theme.txt")).unwrap().contains("fallback system"));
        // a link where the theme goes is refused, never written through
        #[cfg(unix)]
        {
            let other = root.join("other");
            std::fs::create_dir_all(&other).unwrap();
            let s2 = root.join("sounds2");
            std::fs::create_dir_all(&s2).unwrap();
            std::os::unix::fs::symlink(&other, s2.join("original")).unwrap();
            assert!(original_sounds(&data, &s2).is_err());
            assert!(std::fs::read_dir(&other).unwrap().next().is_none());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_size_grows_by_the_factor_up_to_4k() {
        assert_eq!(target(128, 64, 8), (1024, 512));
        assert_eq!(target(1024, 1024, 8), (4096, 4096));
        assert_eq!(target(4096, 4096, 8), (4096, 4096));
    }
}
