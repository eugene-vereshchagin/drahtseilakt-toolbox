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

/// The largest side a texture is made (GPU textures of 8K are common; the originals are 128 to 256).
const SIDE_MAX: u32 = 8192;
/// The smallest side of a texture Real-ESRGAN is given; smaller ones are resampled with Lanczos.
const ESRGAN_MIN_SIDE: u32 = 64;
/// The largest level file and texture read.
const GLTF_MAX: u64 = 64 * 1024 * 1024;
const IMAGE_MAX: u64 = 64 * 1024 * 1024;

/// Converts every original level the copy has that is not in levels/remastered yet, gives every level there its
/// rounder shapes, and upscales its textures by `factor` (1: left as they are). Returns a line a level, and one for
/// each texture left as it was.
pub fn run(data: &Data, factor: u32, model: &str) -> Result<Vec<String>, String> {
    let root = gamedata::profile::dir(gamedata::profile::Kind::Remastered);
    let mut out = Vec::new();
    for n in 1..=data.level_count().unwrap_or(13) {
        let dir = root.join(format!("Level_{n:02}"));
        if std::fs::symlink_metadata(dir.join("level.gltf")).is_ok() {
            continue;
        }
        let Ok(loaded) = data.load(&Source::Original(n)) else {
            continue;
        };
        gamedata::assets::export(data, &loaded, &dir)?;
    }
    let mut levels: Vec<PathBuf> = std::fs::read_dir(&root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| real_dir(p) && real_file(&p.join("level.gltf")))
        .collect();
    levels.sort();
    let esrgan = esrgan();
    if let Some(prog) = &esrgan {
        has_model(prog, model)?;
    }
    let esrgan = esrgan.as_deref().map(|p| (p, model));
    for dir in levels {
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
            .and_then(|n| data.load(&Source::Original(n)).ok());
        match shapes(&dir, original.map(|l| l.level), data) {
            Ok(n) => out.push(format!("{}: {n} meshes rounder", dir.display())),
            Err(e) => out.push(format!("{}: shapes left as they were: {e}", dir.display())),
        }
        if factor < 2 {
            continue;
        }
        match level(&dir, factor, esrgan, &mut out) {
            Ok(n) => out.push(format!(
                "{}: {n} textures x{factor} ({})",
                dir.display(),
                if esrgan.is_some() {
                    "Real-ESRGAN"
                } else {
                    "Lanczos"
                }
            )),
            Err(e) => out.push(format!("{}: not remastered: {e}", dir.display())),
        }
    }
    // the parts every level shares (the balls, the modules, the transformer), remade the same way into .shared/,
    // where the game takes their looks and textures for remastered levels
    let mut parts = vec![
        "3D Entities/Balls.nmo".to_owned(),
        "3D Entities/AnimTrafo.nmo".to_owned(),
    ];
    if let Some(ph) = data.file("3D Entities/PH") {
        let mut names: Vec<String> = std::fs::read_dir(&ph)
            .map_err(|e| format!("{}: {e}", ph.display()))?
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_owned))
            .filter(|n| n.to_ascii_lowercase().ends_with(".nmo"))
            .take(1024)
            .collect();
        names.sort();
        parts.extend(names.into_iter().map(|n| format!("3D Entities/PH/{n}")));
    }
    let (mut made, mut grown) = (0, 0);
    for rel in parts {
        let Some(loaded) = data
            .file(&rel)
            .and_then(|p| gamedata::assets::load_nmo(&p).ok())
        else {
            continue;
        };
        let dir = gamedata::assets::remade_dir(&rel);
        if let Err(e) = real_dirs(&root, &dir.join("textures")) {
            out.push(format!("{rel}: not remade: {e}"));
            continue;
        }
        match shapes(&dir, Some(loaded.level), data) {
            Ok(_) => made += 1,
            Err(e) => {
                out.push(format!("{rel}: not remade: {e}"));
                continue;
            }
        }
        if factor >= 2 {
            match level(&dir, factor, esrgan, &mut out) {
                Ok(n) => grown += n,
                Err(e) => out.push(format!("{rel}: textures left as they were: {e}")),
            }
        }
    }
    out.push(format!(
        "{}: {made} shared parts remade, {grown} of their textures x{factor}",
        root.join(".shared").display()
    ));
    Ok(out)
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
fn shapes(dir: &Path, fresh: Option<level::Level>, data: &Data) -> Result<usize, String> {
    let mut level = match fresh {
        Some(l) => l,
        None => {
            let l = level::open::import(dir)?;
            // read back, its textures are files in the folder; written again they are named as an export names them
            for (i, t) in l.textures.iter().enumerate() {
                let ours = dir
                    .join(level::open::texture_file(i, &t.name))
                    .to_string_lossy()
                    .into_owned();
                if !matches!(&t.source, level::TextureSource::File(p) if *p == ours) {
                    return Err("its textures are not named as the game exports them".into());
                }
            }
            l
        }
    };
    let n = crate::smooth::looks(&mut level);
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
    level::open::export(&level, &tmp)?;
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
    let textures = dir.join("textures");
    for (i, t) in level.textures.iter().enumerate() {
        let path = dir.join(level::open::texture_file(i, &t.name));
        if std::fs::symlink_metadata(&path).is_ok() || !real_dir(&textures) {
            continue;
        }
        if let Some((w, h, rgba)) = data.texture(t) {
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

/// The model is one of Real-ESRGAN's: a plain name with its `.param` and `.bin` in `models/` beside the program.
fn has_model(prog: &Path, model: &str) -> Result<(), String> {
    let plain = !model.is_empty()
        && model.len() <= 64
        && model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !model.starts_with('.');
    let models = prog.parent().map(|d| d.join("models"));
    let there = models.is_some_and(|m| ["param", "bin"].iter().all(|e| real_file(&m.join(format!("{model}.{e}")))));
    if plain && there {
        Ok(())
    } else {
        Err(format!("Real-ESRGAN has no model {model:?} in its models/ folder"))
    }
}

/// Upscales one level's textures; returns how many. A texture that cannot be read or written is reported in `notes`
/// and left as it is.
fn level(
    dir: &Path,
    factor: u32,
    esrgan: Option<(&Path, &str)>,
    notes: &mut Vec<String>,
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
    for img in doc
        .get("images")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
    {
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
            let big = match esrgan.filter(|_| !tiny) {
                Some((prog, model)) => esrgan_upscale(prog, model, &source, factor, &tmp)
                    .unwrap_or_else(|_| lanczos(&source, factor)),
                None => lanczos(&source, factor),
            };
            // the file keeps its own format (what the level file names)
            let format = image::ImageFormat::from_path(&path).map_err(|e| e.to_string())?;
            replace(&path, |t| {
                let img = image::DynamicImage::ImageRgba8(big);
                let img = if format == image::ImageFormat::Jpeg {
                    image::DynamicImage::ImageRgb8(img.to_rgb8())
                } else {
                    img
                };
                img.save_with_format(t, format).map_err(|e| e.to_string())
            })
        };
        match one() {
            Ok(()) => done += 1,
            Err(e) => notes.push(format!("{}: left as it was: {e}", path.display())),
        }
    }
    if real_dir(&tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Ok(done)
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
fn load(path: &Path) -> Result<RgbaImage, String> {
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
    let big = imageops::resize(&padded(img, p), w + 2 * p * f, h + 2 * p * f, imageops::FilterType::Lanczos3);
    let big = imageops::crop_imm(&big, p * f, p * f, w, h).to_image();
    imageops::unsharpen(&big, 0.6 * (f as f32).sqrt(), 2)
}

/// The margin a texture gets around it while it grows: an eighth of its shorter side, at least 4 texels.
fn margin(img: &RgbaImage) -> u32 {
    (img.width().min(img.height()) / 8).max(4).min(img.width().min(img.height()))
}

/// The texture repeats as a tile: across its edges it changes no more than twice as much as within (a decal or a
/// picture does not, its edges do not meet).
fn tiles(img: &RgbaImage) -> bool {
    let (w, h) = img.dimensions();
    if w < 2 || h < 2 {
        return false;
    }
    let d = |a: &image::Rgba<u8>, b: &image::Rgba<u8>| -> u64 {
        a.0.iter().zip(b.0).map(|(x, y)| u64::from(x.abs_diff(y))).sum()
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
        *px = *img.get_pixel(at(i64::from(x) - i64::from(p), w), at(i64::from(y) - i64::from(p), h));
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
        let data = Data { root: root.clone() };
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

    #[test]
    fn the_size_grows_by_the_factor_up_to_8k() {
        assert_eq!(target(128, 64, 8), (1024, 512));
        assert_eq!(target(4096, 4096, 8), (8192, 8192));
        assert_eq!(target(8192, 8192, 8), (8192, 8192));
    }
}
