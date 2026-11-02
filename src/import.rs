// `import-maps`: sorts a downloaded collection of community maps (an unpacked archive: folders of folders, pictures,
// documents, packed archives) into the shape the game lists (the game's docs/drahtseilakt/levels.md, "A level is a
// folder"): one folder per level directly in levels/community/, holding the `.nmo`, a preview picture when one lies
// beside it, and a level.txt with its title and where it came from. The source is a stranger's download: links are
// never followed, depth, counts and sizes are capped, names are made safe for Linux, Windows and macOS, the source is
// only read (copied, never moved), and each level is written into a hidden folder first and renamed into place, so a
// cut-off run leaves no half level the game would list.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// How deep folders are followed, how many entries are looked at, and how many levels one import makes at most.
const DEPTH_MAX: usize = 8;
const ENTRIES_MAX: usize = 200_000;
const LEVELS_MAX: usize = 20_000;
/// The largest `.nmo` copied (the game reads none larger) and the largest preview picture.
const NMO_MAX: u64 = 256 * 1024 * 1024;
const IMAGE_MAX: u64 = 32 * 1024 * 1024;
/// A level folder's name and a title, in characters (the game shows at most 40).
const NAME_MAX: usize = 40;
const TITLE_MAX: usize = 40;

/// What an import did.
#[derive(Debug, Default)]
pub struct Summary {
    /// Level folders made.
    pub imported: usize,
    /// Maps left out as the same file as one imported already: (left out, the one kept).
    pub duplicates: Vec<(PathBuf, PathBuf)>,
    /// Packed archives found and not opened (unpack them and import again).
    pub archives: Vec<PathBuf>,
    /// Links found and not followed.
    pub links: Vec<PathBuf>,
    /// Maps that could not be imported, and why.
    pub failed: Vec<String>,
}

/// One map found: its file and the folder it lies in.
struct Found {
    file: PathBuf,
    size: u64,
}

/// Imports every `.nmo` under `from` into a level folder of its own in `to` (made if missing). `tell` hears a line
/// per level made and per problem; `stop`, once set, ends the import between two levels.
pub fn import(
    from: &Path,
    to: &Path,
    tell: &mut dyn FnMut(String),
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Summary, String> {
    let meta = std::fs::metadata(from).map_err(|e| format!("{}: {e}", from.display()))?;
    if !meta.is_dir() {
        return Err(format!("{}: not a folder (unpack an archive first)", from.display()));
    }
    // checked before anything is made: the source is only ever read
    let src = canonical(from)?;
    let to_be = canonical_to_be(to);
    if src.starts_with(&to_be) || to_be.starts_with(&src) {
        return Err(format!(
            "{} and {} are inside one another",
            from.display(),
            to.display()
        ));
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    if std::fs::symlink_metadata(to)
        .map_err(|e| format!("{}: {e}", to.display()))?
        .file_type()
        .is_symlink()
    {
        return Err(format!("{}: a link; give the folder itself", to.display()));
    }
    let dst = canonical(to)?;
    let mut summary = Summary::default();
    let mut maps = Vec::new();
    let mut seen = 0usize;
    walk(&src, 0, &mut seen, &mut maps, &mut summary)?;
    // in path order, so the same download always gives the same folders and keeps the same one of two copies
    maps.sort_by(|a, b| a.file.cmp(&b.file));
    let mut taken: HashSet<String> = existing_names(&dst);
    // the maps of level folders already there count as imported: a second run adds only what is new
    let mut by_hash: HashMap<(u64, u64), Vec<PathBuf>> = HashMap::new();
    for (file, size) in existing_maps(&dst) {
        if let Ok(b) = read_capped(&file, NMO_MAX) {
            by_hash.entry((fnv1a(&b), size)).or_default().push(file);
        }
    }
    for (n, m) in maps.iter().enumerate() {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(format!("stopped after {} levels", summary.imported));
        }
        if n >= LEVELS_MAX {
            summary
                .failed
                .push(format!("more than {LEVELS_MAX} maps: the rest left out"));
            break;
        }
        let rel = m.file.strip_prefix(&src).unwrap_or(&m.file).to_path_buf();
        let bytes = match read_capped(&m.file, NMO_MAX) {
            Ok(b) => b,
            Err(e) => {
                tell(format!("skipped {}: {e}", rel.display()));
                summary.failed.push(format!("{}: {e}", rel.display()));
                continue;
            }
        };
        // the same file twice (one map in two collections) is one level
        let key = (fnv1a(&bytes), m.size);
        let same = by_hash.get(&key).and_then(|kept| {
            kept.iter()
                .find(|k| read_capped(k, NMO_MAX).is_ok_and(|other| other == bytes))
                .cloned()
        });
        if let Some(kept) = same {
            let kept_rel = kept
                .strip_prefix(&src)
                .or_else(|_| kept.strip_prefix(&dst))
                .unwrap_or(&kept)
                .to_path_buf();
            tell(format!("same as {}: {}", kept_rel.display(), rel.display()));
            summary.duplicates.push((rel, kept_rel));
            continue;
        }
        by_hash.entry(key).or_default().push(m.file.clone());
        let title = title_for(&m.file, &src);
        let name = unique(&title, &mut taken);
        match write_level(&dst, &name, &title, &rel, &bytes, &m.file) {
            Ok(()) => {
                summary.imported += 1;
                tell(format!("{name}  <-  {}", rel.display()));
            }
            Err(e) => {
                tell(format!("failed {}: {e}", rel.display()));
                summary.failed.push(format!("{}: {e}", rel.display()));
            }
        }
    }
    for a in &summary.archives {
        tell(format!(
            "archive not opened (unpack it and import again): {}",
            a.display()
        ));
    }
    for l in &summary.links {
        tell(format!("link not followed: {}", l.display()));
    }
    Ok(summary)
}

/// One line saying what an import did.
pub fn summary_line(s: &Summary, to: &Path) -> String {
    format!(
        "{} levels made in {}; {} the same as another, {} archives not opened, {} links not followed, {} failed",
        s.imported,
        to.display(),
        s.duplicates.len(),
        s.archives.len(),
        s.links.len(),
        s.failed.len()
    )
}

/// Collects the `.nmo` files under `dir`, the archives and the links, never following a link.
fn walk(dir: &Path, depth: usize, seen: &mut usize, maps: &mut Vec<Found>, s: &mut Summary) -> Result<(), String> {
    let read = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut entries: Vec<std::fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        *seen = seen.saturating_add(1);
        if *seen > ENTRIES_MAX {
            return Err(format!(
                "more than {ENTRIES_MAX} files and folders: not a map collection"
            ));
        }
        let path = e.path();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_symlink() {
            s.links.push(path);
        } else if kind.is_dir() {
            if depth + 1 >= DEPTH_MAX {
                s.failed
                    .push(format!("{}: folders nested too deep, not looked into", path.display()));
            } else {
                walk(&path, depth + 1, seen, maps, s)?;
            }
        } else if kind.is_file() {
            if has_ext(&path, &["nmo"]) {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                maps.push(Found { file: path, size });
            } else if is_archive(&path) {
                s.archives.push(path);
            }
        }
    }
    Ok(())
}

fn is_archive(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    [
        ".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2", ".xz", ".zst", ".cab", ".lzh",
    ]
    .iter()
    .any(|x| name.ends_with(x))
}

fn has_ext(p: &Path, exts: &[&str]) -> bool {
    p.extension()
        .is_some_and(|x| exts.iter().any(|e| x.eq_ignore_ascii_case(e)))
}

/// A map's title: its file's name (without a BML-style ".Level"), or, for a generic name in a collection
/// (Level_07.NMO in "45-Degree Rotated Maps"), the collection's name and the level's number.
fn title_for(file: &Path, root: &Path) -> String {
    let stem = file
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = match stem.len().checked_sub(6) {
        Some(at) if stem.get(at..).is_some_and(|t| t.eq_ignore_ascii_case(".level")) => {
            stem.get(..at).unwrap_or_default().to_owned()
        }
        _ => stem,
    };
    let title = match generic_number(&stem) {
        Some(n) => {
            let folder = file.parent().filter(|p| *p != root).or(Some(root));
            let collection = folder
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned());
            let collection = collection.map(|c| strip_maps(&c)).filter(|c| !c.is_empty());
            match collection {
                Some(c) => format!("{c} - Level {n:02}"),
                None => format!("Level {n:02}"),
            }
        }
        None => stem,
    };
    clean(&title, usize::MAX)
}

/// The number of a generic level name ("Level_07", "level 7", "Level07", "07"), if it is one.
fn generic_number(stem: &str) -> Option<u32> {
    let s = stem.trim();
    let rest = if s.len() >= 5 && s.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("level")) {
        s.get(5..)?.trim_start_matches(['_', '-', ' '])
    } else {
        s
    };
    (!rest.is_empty() && rest.len() <= 3 && rest.bytes().all(|b| b.is_ascii_digit())).then(|| rest.parse().ok())?
}

/// A collection's name without a trailing "Maps" ("Invisible Maps" -> "Invisible").
fn strip_maps(c: &str) -> String {
    let t = c.trim();
    match t.len().checked_sub(4) {
        Some(at) if at > 0 && t.get(at..).is_some_and(|x| x.eq_ignore_ascii_case("maps")) => {
            t.get(..at).unwrap_or(t).trim_end_matches([' ', '-', '_']).to_owned()
        }
        _ => t.to_owned(),
    }
}

/// Text without control, format and direction characters, spaces made one, trimmed, at most `max` characters (a cut
/// one at a word, ending in "…"): as the game cleans what it shows.
fn clean(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if c.is_control() || is_format(c) {
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    if out.chars().count() <= max {
        return out;
    }
    cut_words(&out, max)
}

/// `s` in at most `max` characters: when longer, cut at the last word boundary that leaves it at least half its
/// room (else in the word), separators at the cut dropped, and ended with "…".
fn cut_words(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_owned();
    }
    let room = max.saturating_sub(1);
    let at_space = (room / 2..=room)
        .rev()
        .find(|&i| chars.get(i).is_some_and(|c| c.is_whitespace()));
    let head: String = chars.iter().take(at_space.unwrap_or(room)).collect();
    let head = head.trim_end_matches(|c: char| c.is_whitespace() || "-–—,;:(/.".contains(c));
    format!("{head}…")
}

fn is_format(c: char) -> bool {
    matches!(u32::from(c),
        0x00AD | 0x034F | 0x061C | 0x115F | 0x1160 | 0x17B4 | 0x17B5 | 0x180B..=0x180F | 0x200B..=0x200F
        | 0x2028..=0x202F | 0x205F..=0x206F | 0x3164 | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0xFFF0..=0xFFFB
        | 0x1D173..=0x1D17A | 0xE0000..=0xE0FFF)
}

/// A folder name every system takes: none of `<>:"/\|?*`, no control characters, no leading dot (the game hides
/// those) or space, no trailing dot or space, not a reserved device name of Windows, at most NAME_MAX characters.
fn folder_name(title: &str) -> String {
    fit_name(title, NAME_MAX)
}

/// `folder_name` in at most `max` characters, a longer one cut at a word and ended in "…".
fn fit_name(title: &str, max: usize) -> String {
    let replaced: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let name = clean(&replaced, usize::MAX);
    let name = name.trim_start_matches(['.', ' ']);
    // "…" is no dot, so a cut name keeps it at its end on every system; level.txt keeps the whole title
    let name = cut_words(name, max);
    let name = name
        .trim_start_matches(['.', ' '])
        .trim_end_matches(['.', ' '])
        .to_owned();
    let name = if name.is_empty() { "Level".to_owned() } else { name };
    if reserved(&name) { format!("{name}_") } else { name }
}

/// Windows' device names, alone or before an extension ("CON", "nul.txt", "COM1", "LPT9.nmo").
fn reserved(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name).trim_end().to_ascii_uppercase();
    let numbered = |p: &str| {
        base.strip_prefix(p).is_some_and(|d| {
            matches!(
                d,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0" | "¹" | "²" | "³"
            )
        })
    };
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") || numbered("COM") || numbered("LPT")
}

/// The folder name for `title`, or with ` (2)`, ` (3)`... when that (in any case) is taken, the title cut shorter to
/// leave the number its room; the one returned is taken.
fn unique(title: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = folder_name(title);
    let mut n = 2u32;
    while taken.contains(&candidate.to_lowercase()) {
        let suffix = format!(" ({n})");
        candidate = format!(
            "{}{suffix}",
            fit_name(title, NAME_MAX.saturating_sub(suffix.chars().count()))
        );
        n = n.saturating_add(1);
    }
    taken.insert(candidate.to_lowercase());
    candidate
}

/// The `.nmo` files of the level folders in `dir` (directly in each, links passed over), with their sizes.
fn existing_maps(dir: &Path) -> Vec<(PathBuf, u64)> {
    let plain =
        |e: &std::fs::DirEntry, dir: bool| e.file_type().is_ok_and(|k| if dir { k.is_dir() } else { k.is_file() });
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .take(ENTRIES_MAX)
        .filter(|e| plain(e, true))
        .filter_map(|e| std::fs::read_dir(e.path()).ok())
        .flat_map(|r| r.flatten().take(4096).collect::<Vec<_>>())
        .filter(|e| plain(e, false) && has_ext(&e.path(), &["nmo"]))
        .map(|e| (e.path(), e.metadata().map(|m| m.len()).unwrap_or(0)))
        .collect()
}

fn existing_names(dir: &Path) -> HashSet<String> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// Writes the level folder `to/name`: the map, its preview, level.txt; into a hidden folder first, renamed into
/// place when complete.
fn write_level(to: &Path, name: &str, title: &str, rel: &Path, bytes: &[u8], file: &Path) -> Result<(), String> {
    let fail = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let tmp = to.join(format!(".import-{}-{}", std::process::id(), fnv1a(name.as_bytes())));
    if std::fs::symlink_metadata(&tmp).is_ok() {
        std::fs::remove_dir_all(&tmp).map_err(|e| fail(&tmp, e))?;
    }
    std::fs::create_dir(&tmp).map_err(|e| fail(&tmp, e))?;
    let result = (|| {
        // the map keeps its file's name where it is safe (the game takes an original level's settings by it)
        let stem = file
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = file
            .extension()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "nmo".into());
        let map_name = format!("{}.{ext}", folder_name(&stem));
        write_new(&tmp.join(&map_name), bytes)?;
        if let Some(img) = preview(file) {
            let ext = img
                .extension()
                .map(|x| x.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let ext = if ext == "jpeg" { "jpg".to_owned() } else { ext };
            if let Ok(b) = read_capped(&img, IMAGE_MAX) {
                write_new(&tmp.join(format!("preview.{ext}")), &b)?;
            }
        }
        let mut info = String::from("# Shown in the game's list of levels; edit as you like.\n");
        // the whole title (the game shows its first TITLE_MAX characters) when the folder's name had to change
        // characters; a numbered folder (a second "Hello Mars") keeps its number on screen instead
        let numbered = name != folder_name(title);
        if !numbered && clean(title, TITLE_MAX) != name {
            info.push_str(&format!("title = {}\n", clean(title, 200)));
        }
        let source = clean(&rel.to_string_lossy(), 400);
        info.push_str(&format!("source = {source}\n"));
        write_new(&tmp.join("level.txt"), info.as_bytes())
    })();
    let result = result.and_then(|()| {
        let dst = to.join(name);
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(format!("{}: already there", dst.display()));
        }
        std::fs::rename(&tmp, &dst).map_err(|e| fail(&dst, e))
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    result
}

/// A picture of the map beside it: one named as the map, else one named as the map's folder (a season's folder
/// for one track, with its picture and the track's variants).
fn preview(file: &Path) -> Option<PathBuf> {
    let dir = file.parent()?;
    let stem = file.file_stem()?.to_string_lossy().to_lowercase();
    let folder = dir.file_name()?.to_string_lossy().to_lowercase();
    let mut images: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .take(4096)
        .filter(|e| e.file_type().is_ok_and(|k| k.is_file()))
        .map(|e| e.path())
        .filter(|p| has_ext(p, &["jpg", "jpeg", "png"]))
        .collect();
    images.sort();
    let named = |want: &str| {
        images
            .iter()
            .find(|p| {
                p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().to_lowercase() == want)
            })
            .cloned()
    };
    named(&stem).or_else(|| named(&folder))
}

/// A new file (never one already there, never through a link).
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A plain file's bytes, refused when it is a link or larger than `cap`.
fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let meta = std::fs::symlink_metadata(path).map_err(fail)?;
    if !meta.is_file() {
        return Err(format!("{}: not a plain file", path.display()));
    }
    if meta.len() > cap {
        return Err(format!("{}: larger than {} MiB", path.display(), cap / (1024 * 1024)));
    }
    let file = std::fs::File::open(path).map_err(fail)?;
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes).map_err(fail)?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > cap) {
        return Err(format!("{}: grew past {} MiB", path.display(), cap / (1024 * 1024)));
    }
    Ok(bytes)
}

/// A folder that may not exist yet as it will be: its nearest existing parent made canonical, the rest added.
fn canonical_to_be(p: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = p;
    loop {
        if let Ok(c) = std::fs::canonicalize(at) {
            return rest.iter().rev().fold(c, |acc: PathBuf, part| acc.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                at = parent;
            }
            _ => return p.to_path_buf(),
        }
    }
}

fn canonical(p: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    gamedata::fnv1a(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    static GO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("toolbox-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names_are_safe_everywhere() {
        assert_eq!(folder_name("a<b>c:d\"e/f\\g|h?i*j"), "a b c d e f g h i j");
        assert_eq!(folder_name("  ..hidden. . "), "hidden");
        assert_eq!(folder_name("CON"), "CON_");
        assert_eq!(folder_name("nul.txt"), "nul.txt_");
        assert_eq!(folder_name("COM1"), "COM1_");
        assert_eq!(folder_name("Console"), "Console");
        assert_eq!(folder_name("\u{202e}\u{7}"), "Level");
        let long = folder_name(&"Long title ".repeat(10));
        assert!(long.chars().count() <= NAME_MAX && !long.ends_with(' '));
        // names alike but for their ends stay apart
        let a = folder_name("Ballance Skyscraper (3D Maze; Hard Ver.-Fortress Besieged) v1.3.2");
        let b = folder_name("Ballance Skyscraper (3D Maze; Hard Ver.-Blind) v1.2.2");
        assert_eq!(
            (a.as_str(), b.as_str()),
            (
                "Ballance Skyscraper (3D Maze; Hard…",
                "Ballance Skyscraper (3D Maze; Hard…"
            )
        );
        let mut taken = HashSet::new();
        assert_eq!(unique("Map", &mut taken), "Map");
        assert_eq!(unique("map", &mut taken), "map (2)");
        // a second long title alike: cut shorter, at a word, before a readable number
        let long = "Ballance Skyscraper (3D Maze; Hard Ver.-Blind) v1.2.2";
        assert_eq!(unique(long, &mut taken), "Ballance Skyscraper (3D Maze; Hard…");
        assert_eq!(unique(long, &mut taken), "Ballance Skyscraper (3D Maze; Hard… (2)");
    }

    #[test]
    fn titles_come_from_the_file_and_the_collection() {
        let root = Path::new("/r");
        assert_eq!(
            title_for(Path::new("/r/45-Degree Rotated Maps/Level_07.NMO"), root),
            "45-Degree Rotated - Level 07"
        );
        assert_eq!(
            title_for(Path::new("/r/Invisible Maps/level 3.nmo"), root),
            "Invisible - Level 03"
        );
        assert_eq!(title_for(Path::new("/r/S1/W天空峡谷.Level.NMO"), root), "W天空峡谷");
        assert_eq!(
            title_for(Path::new("/r/Custom Maps/Hello Mars.NMO"), root),
            "Hello Mars"
        );
        assert_eq!(title_for(Path::new("/r/反向Level_01.NMO"), root), "反向Level_01");
        assert_eq!(generic_number("Level_13"), Some(13));
        assert_eq!(generic_number("Levels"), None);
        assert_eq!(generic_number("Level_1234"), None);
    }

    #[test]
    fn a_messy_collection_becomes_level_folders() {
        let src = temp("src");
        let dst = temp("dst");
        let mk = |p: &str, b: &[u8]| {
            let p = src.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        mk("Rotated Maps/Level_01.NMO", b"r1");
        mk("Rotated Maps/Level_02.NMO", b"r2");
        mk("Custom Maps/Hello Mars.NMO", b"mars");
        mk("Custom Maps/Hello Mars.jpg", b"jpg");
        mk("Custom Maps/Packed.tar.gz", b"gz");
        mk("Other/Again/Hello Mars.NMO", b"mars");
        mk("Other/hello mars.nmo", b"different");
        mk("Season/Track/TrackW2.NMO", b"t2");
        mk("Season/Track/Track.jpg", b"tj");
        mk("Season/rules.doc", b"doc");
        mk("CON.nmo", b"con");
        mk("Odd: name?.nmo", b"odd");
        let mut lines = Vec::new();
        let s = import(&src, &dst, &mut |l| lines.push(l), &GO).unwrap();
        assert_eq!(s.imported, 7, "{lines:?}");
        let odd = std::fs::read_to_string(dst.join("Odd name/level.txt")).unwrap();
        assert!(odd.contains("title = Odd: name?\n"), "{odd}");
        assert_eq!(s.duplicates.len(), 1);
        assert_eq!(s.archives.len(), 1);
        let mut names: Vec<String> = std::fs::read_dir(&dst)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "CON_",
                "Hello Mars",
                "Odd name",
                "Rotated - Level 01",
                "Rotated - Level 02",
                "TrackW2",
                "hello mars (2)"
            ]
        );
        assert_eq!(std::fs::read(dst.join("Hello Mars/preview.jpg")).unwrap(), b"jpg");
        assert_eq!(std::fs::read(dst.join("TrackW2/preview.jpg")).unwrap(), b"tj");
        assert_eq!(std::fs::read(dst.join("Hello Mars/Hello Mars.NMO")).unwrap(), b"mars");
        let info = std::fs::read_to_string(dst.join("hello mars (2)/level.txt")).unwrap();
        assert!(!info.contains("title") && info.contains("source = Other"), "{info}");
        // the source is untouched, and a second run makes new names beside the first
        assert!(src.join("Custom Maps/Hello Mars.NMO").exists());
        let again = import(&src, &dst, &mut |_| {}, &GO).unwrap();
        assert_eq!((again.imported, again.duplicates.len()), (0, 8));
        assert!(dst.join("CON_/CON_.nmo").exists());
        // no hidden work folder is left
        assert!(
            std::fs::read_dir(&dst)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with('.'))
        );
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    #[test]
    fn nested_folders_are_refused() {
        let src = temp("nest");
        assert!(import(&src, &src.join("out"), &mut |_| {}, &GO).is_err());
        assert!(!src.join("out").exists(), "nothing is made in the source");
        let _ = std::fs::remove_dir_all(&src);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_followed() {
        use std::os::unix::fs::symlink;
        let src = temp("lsrc");
        let dst = temp("ldst");
        let outside = temp("lout");
        std::fs::write(outside.join("secret.nmo"), b"s").unwrap();
        std::fs::create_dir_all(outside.join("dir")).unwrap();
        std::fs::write(outside.join("dir/x.nmo"), b"x").unwrap();
        symlink(outside.join("secret.nmo"), src.join("linked.nmo")).unwrap();
        symlink(outside.join("dir"), src.join("linked dir")).unwrap();
        std::fs::write(src.join("real.nmo"), b"r").unwrap();
        let s = import(&src, &dst, &mut |_| {}, &GO).unwrap();
        assert_eq!((s.imported, s.links.len()), (1, 2));
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
