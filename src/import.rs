// `import-maps`: sorts a downloaded collection of community maps (an unpacked archive: folders of folders, pictures,
// documents, packed archives) into the shape the game lists (the game's docs/drahtseilakt/levels.md, "A level is a
// folder"): one folder per level directly in levels/downloaded/, holding the `.nmo`, a preview picture when one lies
// beside it, and a level.txt with its title and where it came from. The source is a stranger's download: links are
// never followed, depth, counts and sizes are capped, names are made safe for Linux, Windows and macOS, the source is
// only read (copied, never moved), and each level is written into a hidden folder first and renamed into place, so a
// cut-off run leaves no half level the game would list.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::catalogue::Catalogue;
use crate::unsorted::{Reason, Unsorted};

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
    /// Maps of the modified originals (a 45-degree, reverse or invisible set) sorted into levels/modified.
    pub modified: usize,
    /// Of the levels made: found in the catalogue, and kept for a readme beside them alone.
    pub catalogued: usize,
    pub readme_only: usize,
    /// Items set aside into the unsorted folder: (path in it, why).
    pub set_aside: Vec<(String, Reason)>,
}

/// How an import sorts: the catalogue (with one, a map neither in it nor with a readme beside it is set aside) and the
/// folder things that do not fit are copied into.
pub struct Sorting<'a> {
    pub catalogue: Option<&'a Catalogue>,
    pub unsorted: PathBuf,
}

/// The unsorted folder beside a levels folder: `levels/downloaded` -> `levels/downloaded-unsorted`.
pub fn default_unsorted(to: &Path) -> PathBuf {
    let parent = to
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent.join("community-unsorted")
}

/// The modified originals folder beside a levels folder: `levels/downloaded` -> `levels/modified`.
pub fn default_modified(to: &Path) -> PathBuf {
    let parent = to
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent.join("modified")
}

/// The set folder (as the game's modified.rs knows them: 45-degree / rotated, reverse, invisible, by name, in
/// English or Chinese) a map lies in: the outermost such folder on its way below the download's folder, or the
/// download's own folder. "Pro Racing" and the like are none: they are new geometry and stay community maps.
fn modified_set(file: &Path, root: &Path) -> Option<String> {
    let is_set = |n: &str| {
        let l = n.to_lowercase();
        [
            "45-degree",
            "rotat",
            "旋转",
            "revers",
            "反向",
            "invisib",
            "隐形",
        ]
        .iter()
        .any(|w| l.contains(w))
    };
    let inside = file.strip_prefix(root).ok()?.parent()?;
    let own = root.file_name().map(|n| n.to_string_lossy().into_owned());
    inside
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .chain(own)
        .find(|n| is_set(n))
}

/// Writes a modified original as `modified/<set>/<file stem>/<file>.nmo` (and its preview), through a hidden folder
/// renamed into place. A folder name taken by another map gets ` (2)`, ...; the same map again (an earlier run) is left
/// alone: None. Returns the folder's path below `modified`.
fn write_modified(
    modified: &Path,
    set: &str,
    file: &Path,
    bytes: &[u8],
) -> Result<Option<PathBuf>, String> {
    let fail = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let set_dir = modified.join(folder_name(set));
    std::fs::create_dir_all(&set_dir).map_err(|e| fail(&set_dir, e))?;
    let stem = file
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = folder_name(&stem);
    let mut name = base.clone();
    let mut n = 2u32;
    while std::fs::symlink_metadata(set_dir.join(&name)).is_ok() {
        let there = set_dir
            .join(&name)
            .join(format!("{}.nmo", folder_name(&stem)));
        if read_capped(&there, NMO_MAX).is_ok_and(|b| b == bytes) {
            return Ok(None);
        }
        name = format!("{} ({n})", fit_name(&stem, NAME_MAX.saturating_sub(5)));
        n = n.saturating_add(1);
        if n > 999 {
            return Err(format!("{}: no free folder name", set_dir.display()));
        }
    }
    let tmp = set_dir.join(format!(
        ".import-{}-{}",
        std::process::id(),
        fnv1a(name.as_bytes())
    ));
    if std::fs::symlink_metadata(&tmp).is_ok() {
        std::fs::remove_dir_all(&tmp).map_err(|e| fail(&tmp, e))?;
    }
    std::fs::create_dir(&tmp).map_err(|e| fail(&tmp, e))?;
    let result = (|| {
        write_new(&tmp.join(format!("{}.nmo", folder_name(&stem))), bytes)?;
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
        let dst = set_dir.join(&name);
        std::fs::rename(&tmp, &dst).map_err(|e| fail(&dst, e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    result.map(|()| Some(PathBuf::from(folder_name(set)).join(name)))
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
    sorting: &Sorting,
    tell: &mut dyn FnMut(String),
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Summary, String> {
    let meta = std::fs::metadata(from).map_err(|e| format!("{}: {e}", from.display()))?;
    if !meta.is_dir() {
        return Err(format!(
            "{}: not a folder (unpack an archive first)",
            from.display()
        ));
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
    let aside = canonical_to_be(&sorting.unsorted);
    if aside.starts_with(&src)
        || src.starts_with(&aside)
        || aside.starts_with(&to_be)
        || to_be.starts_with(&aside)
    {
        return Err(format!(
            "{}: the unsorted folder must be beside, not inside, {} and {}",
            sorting.unsorted.display(),
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
    let mut unsorted = Unsorted::new(&sorting.unsorted);
    for a in summary.archives.clone() {
        let rel = a.strip_prefix(&src).unwrap_or(&a).to_path_buf();
        match unsorted.copy(&src, &a, Reason::Archive) {
            Ok(()) => tell(format!("set aside (packed archive): {}", shown(&rel))),
            Err(e) => summary.failed.push(format!("{}: {e}", rel.display())),
        }
    }
    for (n, m) in maps.iter().enumerate() {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = unsorted.finish();
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
                tell(format!("skipped {}: {}", shown(&rel), printable(&e)));
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
            tell(format!("same as {}: {}", shown(&kept_rel), shown(&rel)));
            summary.duplicates.push((rel, kept_rel));
            continue;
        }
        // a map of a modified originals set goes to levels/modified/<set>/<stem>/, whatever the catalogue says
        if let Some(set) = modified_set(&m.file, &src) {
            let modified = default_modified(&dst);
            match write_modified(&modified, &set, &m.file, &bytes) {
                Ok(None) => {
                    tell(format!("already in modified: {}", shown(&rel)));
                    summary
                        .duplicates
                        .push((rel.clone(), PathBuf::from("modified")));
                }
                Ok(Some(at)) => {
                    by_hash.entry(key).or_default().push(m.file.clone());
                    summary.modified += 1;
                    tell(format!(
                        "modified/{}  <-  {}",
                        printable(&at.to_string_lossy()),
                        shown(&rel)
                    ));
                }
                Err(e) => {
                    tell(format!("failed {}: {}", shown(&rel), printable(&e)));
                    summary.failed.push(format!("{}: {e}", rel.display()));
                }
            }
            continue;
        }
        let names = names_of(&m.file, &src);
        let names: Vec<&str> = names.iter().map(AsRef::as_ref).collect();
        let found = sorting.catalogue.and_then(|c| c.find(&names));
        let readme = found.is_none() && has_readme(&m.file);
        if sorting.catalogue.is_some() && found.is_none() && !readme {
            match unsorted.copy(&src, &m.file, Reason::Uncatalogued) {
                Ok(()) => tell(format!(
                    "set aside (not in the catalogue, no readme): {}",
                    shown(&rel)
                )),
                Err(e) => summary.failed.push(format!("{}: {e}", rel.display())),
            }
            continue;
        }
        let (title, lines) = match &found {
            Some(f) => (clean(f.title(), usize::MAX), f.lines()),
            None => (title_for(&m.file, &src), Vec::new()),
        };
        match write_named(&dst, &title, &mut taken, &rel, &bytes, &m.file, &lines) {
            Ok(name) => {
                // counted as imported only once it is there
                by_hash.entry(key).or_default().push(m.file.clone());
                summary.imported += 1;
                if found.is_some() {
                    summary.catalogued += 1;
                } else if readme {
                    summary.readme_only += 1;
                }
                tell(format!("{}  <-  {}", printable(&name), shown(&rel)));
            }
            Err(e) => {
                let e = match e {
                    Written::Exists => "no free folder name".to_owned(),
                    Written::Failed(e) => e,
                };
                tell(format!("failed {}: {}", shown(&rel), printable(&e)));
                summary.failed.push(format!("{}: {e}", rel.display()));
            }
        }
    }
    for l in &summary.links {
        tell(format!("link not followed: {}", shown(l)));
    }
    if let Err(e) = unsorted.finish() {
        summary.failed.push(e);
    }
    summary.set_aside = unsorted.items().to_vec();
    Ok(summary)
}

/// The names a map is looked for by in the catalogue: its file's, then its folder's (a season's folder of one track),
/// not the download's own.
fn names_of<'a>(file: &'a Path, root: &Path) -> Vec<std::borrow::Cow<'a, str>> {
    let mut names = Vec::new();
    if let Some(stem) = file.file_stem() {
        names.push(stem.to_string_lossy());
    }
    if let Some(dir) = file.parent().filter(|p| *p != root)
        && let Some(folder) = dir.file_name()
        && maps_in(dir) == 1
    {
        names.push(folder.to_string_lossy());
    }
    names
}

/// How many plain `.nmo` files lie directly in `dir` (a folder's name stands for its map only when it holds one).
fn maps_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |r| {
        r.flatten()
            .take(4096)
            .filter(|e| e.file_type().is_ok_and(|k| k.is_file()) && has_ext(&e.path(), &["nmo"]))
            .count()
    })
}

/// The documents counted as a map's readme: a text file, or a document named as a readme or as the map.
const DOCS: [&str; 10] = [
    "txt", "md", "nfo", "rtf", "doc", "docx", "odt", "pdf", "htm", "html",
];
const README_NAMES: [&str; 12] = [
    "readme",
    "read me",
    "read_me",
    "read-me",
    "说明",
    "自述",
    "介绍",
    "liesmich",
    "lisezmoi",
    "leeme",
    "прочти",
    "описание",
];

/// Whether `name` (a file beside a map whose file is named `stem`) is a readme: any .txt, .md or .nfo but a level's
/// own level.txt, or a document (.doc, .pdf...) named as a readme or as the map.
fn is_readme(name: &str, stem: &str) -> bool {
    let lower = name.to_lowercase();
    let Some((base, ext)) = lower.rsplit_once('.') else {
        return false;
    };
    if !DOCS.contains(&ext) || lower == "level.txt" || base.is_empty() {
        return false;
    }
    matches!(ext, "txt" | "md" | "nfo")
        || README_NAMES.iter().any(|r| base.contains(r))
        || base == stem.to_lowercase()
}

/// Whether a plain file beside the map `file` is a readme.
fn has_readme(file: &Path) -> bool {
    let (Some(dir), Some(stem)) = (file.parent(), file.file_stem()) else {
        return false;
    };
    let stem = stem.to_string_lossy();
    folder_has_readme(dir, &stem)
}

fn folder_has_readme(dir: &Path, stem: &str) -> bool {
    let Ok(read) = std::fs::read_dir(dir) else {
        return false;
    };
    read.flatten()
        .take(4096)
        .filter(|e| e.file_type().is_ok_and(|k| k.is_file()))
        .any(|e| is_readme(&e.file_name().to_string_lossy(), stem))
}

/// How often a map tries the next numbered name when the disk has its name in another form.
const NAME_TRIES: usize = 100;

/// Why a level folder was not written.
#[derive(Debug)]
enum Written {
    /// A folder of that name is there (perhaps in another case or Unicode form).
    Exists,
    Failed(String),
}

/// A name safe to print on a terminal: control and invisible format characters (escape sequences, direction
/// overrides) shown as `\u{..}` escapes.
pub(crate) fn printable(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || is_format(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn shown(p: &Path) -> String {
    printable(&p.to_string_lossy())
}

/// One line saying what an import did.
pub fn summary_line(s: &Summary, to: &Path) -> String {
    format!(
        "{} levels made in {} ({} from the catalogue, {} for a readme), {} modified originals sorted into levels/modified; {} the same as another, {} set aside ({} packed archives, {} not in the catalogue and without a readme), {} links not followed, {} failed",
        s.imported,
        shown(to),
        s.catalogued,
        s.readme_only,
        s.modified,
        s.duplicates.len(),
        s.set_aside.len(),
        s.set_aside
            .iter()
            .filter(|(_, r)| *r == Reason::Archive)
            .count(),
        s.set_aside
            .iter()
            .filter(|(_, r)| *r == Reason::Uncatalogued)
            .count(),
        s.links.len(),
        s.failed.len()
    )
}

/// The game's program: the one beside the toolbox, else one on the PATH.
fn game_program() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "drahtseilakt.exe"
    } else {
        "drahtseilakt"
    };
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join(name)));
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(name));
    beside.into_iter().chain(on_path).find(|f| f.is_file())
}

/// Has the game check the maps just made in `to` and say, per map, what it does not support yet in it; the game writes
/// each map's report.txt (its audit is the source of truth, the toolbox only asks) and `say` gets its lines (a map's
/// title, then its verdict and the lines under it). A game that is not there or a check that fails only costs the
/// report: the maps are imported either way.
pub fn check_maps(to: &Path, say: &mut dyn FnMut(String)) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let Some(game) = game_program() else {
        say(format!(
            "map check skipped: the game (drahtseilakt) is not beside the toolbox or on the PATH; run `drahtseilakt --audit-report {}` later",
            shown(to)
        ));
        return;
    };
    say("checking the maps for what the game does not support yet ...".to_owned());
    let child = Command::new(game)
        .arg("--audit-report")
        .arg(to)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return say(format!("map check could not start: {e}")),
    };
    if let Some(out) = child.stdout.take() {
        // the game's own words (its language), a line at a time, capped: its bookkeeping lines are left out
        for l in std::io::BufReader::new(out)
            .lines()
            .take(100_000)
            .map_while(Result::ok)
        {
            if !l.starts_with("paths:") && !l.starts_with("report:") {
                say(printable(&l));
            }
        }
    }
    match child.wait() {
        Ok(st) if st.success() => {}
        Ok(st) => say(format!(
            "map check ended with {st}: some maps have no report"
        )),
        Err(e) => say(format!("map check failed: {e}")),
    }
}

/// Collects the `.nmo` files under `dir`, the archives and the links, never following a link.
fn walk(
    dir: &Path,
    depth: usize,
    seen: &mut usize,
    maps: &mut Vec<Found>,
    s: &mut Summary,
) -> Result<(), String> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        // an unreadable folder inside the download is skipped and reported; the download itself must be readable
        Err(e) if depth > 0 => {
            s.failed
                .push(format!("{}: {e}, not looked into", dir.display()));
            return Ok(());
        }
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    // never more than the cap is gathered, however large the folder
    let room = ENTRIES_MAX.saturating_sub(*seen).saturating_add(1);
    let mut entries: Vec<std::fs::DirEntry> = read.flatten().take(room).collect();
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
                s.failed.push(format!(
                    "{}: folders nested too deep, not looked into",
                    path.display()
                ));
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
        Some(at)
            if stem
                .get(at..)
                .is_some_and(|t| t.eq_ignore_ascii_case(".level")) =>
        {
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
    (!rest.is_empty() && rest.len() <= 3 && rest.bytes().all(|b| b.is_ascii_digit()))
        .then(|| rest.parse().ok())?
}

/// A collection's name without a trailing "Maps" ("Invisible Maps" -> "Invisible").
fn strip_maps(c: &str) -> String {
    let t = c.trim();
    match t.len().checked_sub(4) {
        Some(at) if at > 0 && t.get(at..).is_some_and(|x| x.eq_ignore_ascii_case("maps")) => t
            .get(..at)
            .unwrap_or(t)
            .trim_end_matches([' ', '-', '_'])
            .to_owned(),
        _ => t.to_owned(),
    }
}

/// Text without control, format and direction characters, spaces made one, trimmed, at most `max` characters (a cut
/// one at a word, ending in "…"): as the game cleans what it shows.
pub(crate) fn clean(s: &str, max: usize) -> String {
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
    let name = fit_plain(title, max);
    if !reserved(&name) {
        return name;
    }
    // a device name gets "_" after its base, before any extension ("nul.txt" -> "nul_.txt", "Aux.Level.nmo" ->
    // "Aux_.Level.nmo"), within the cap
    let name = if name.chars().count() >= max && max > 1 {
        fit_plain(title, max - 1)
    } else {
        name
    };
    let at = name.find('.').unwrap_or(name.len());
    format!(
        "{}_{}",
        name.get(..at).unwrap_or_default(),
        name.get(at..).unwrap_or_default()
    )
}

/// `fit_name` without the device names' "_".
fn fit_plain(title: &str, max: usize) -> String {
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
    if name.is_empty() {
        "Level".to_owned()
    } else {
        name
    }
}

/// Windows' device names, alone or before an extension ("CON", "nul.txt", "COM1", "LPT9.nmo").
fn reserved(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    let numbered = |p: &str| {
        base.strip_prefix(p).is_some_and(|d| {
            matches!(
                d,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "0" | "¹" | "²" | "³"
            )
        })
    };
    matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered("COM")
        || numbered("LPT")
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
    let plain = |e: &std::fs::DirEntry, dir: bool| {
        e.file_type()
            .is_ok_and(|k| if dir { k.is_dir() } else { k.is_file() })
    };
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

/// Writes the level folder for `title` under the first free name: a name the disk already has in another form
/// (case, Unicode normalisation), unknown to `taken`, takes the next number.
fn write_named(
    to: &Path,
    title: &str,
    taken: &mut HashSet<String>,
    rel: &Path,
    bytes: &[u8],
    file: &Path,
    lines: &[String],
) -> Result<String, Written> {
    let mut outcome = Err(Written::Exists);
    for _ in 0..NAME_TRIES {
        let name = unique(title, taken);
        outcome = write_level(to, &name, title, rel, bytes, file, lines).map(|()| name);
        if !matches!(outcome, Err(Written::Exists)) {
            break;
        }
    }
    outcome
}

/// Writes the level folder `to/name`: the map, its preview, level.txt; into a hidden folder first, renamed into
/// place when complete.
fn write_level(
    to: &Path,
    name: &str,
    title: &str,
    rel: &Path,
    bytes: &[u8],
    file: &Path,
    lines: &[String],
) -> Result<(), Written> {
    // a name the disk has in another case or Unicode form is found here (case-insensitive and normalising systems)
    if std::fs::symlink_metadata(to.join(name)).is_ok() {
        return Err(Written::Exists);
    }
    write_level_files(to, name, title, rel, bytes, file, lines).map_err(|e| {
        if std::fs::symlink_metadata(to.join(name)).is_ok() {
            Written::Exists
        } else {
            Written::Failed(e)
        }
    })
}

/// The comment level.txt starts with.
const INFO_HEADER: &str = "# Shown in the game's list of levels; edit as you like.";

fn write_level_files(
    to: &Path,
    name: &str,
    title: &str,
    rel: &Path,
    bytes: &[u8],
    file: &Path,
    lines: &[String],
) -> Result<(), String> {
    let fail = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let tmp = to.join(format!(
        ".import-{}-{}",
        std::process::id(),
        fnv1a(name.as_bytes())
    ));
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
        let mut info = format!("{INFO_HEADER}\n");
        if lines.is_empty() {
            // the whole title (the game shows its first TITLE_MAX characters) when the folder's name had to change
            // characters; a numbered folder (a second "Hello Mars") keeps its number on screen instead
            let numbered = name != folder_name(title);
            if !numbered && clean(title, TITLE_MAX) != name {
                info.push_str(&format!("title = {}\n", clean(title, 200)));
            }
        }
        // the catalogue's title, Chinese title, author and difficulty
        for l in lines {
            info.push_str(l);
            info.push('\n');
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
    f.write_all(bytes)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A plain file's bytes, refused when it is a link or larger than `cap`.
fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let meta = std::fs::symlink_metadata(path).map_err(fail)?;
    if !meta.is_file() {
        return Err(format!("{}: not a plain file", path.display()));
    }
    if meta.len() > cap {
        return Err(format!(
            "{}: larger than {} MiB",
            path.display(),
            cap / (1024 * 1024)
        ));
    }
    let file = std::fs::File::open(path).map_err(fail)?;
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > cap) {
        return Err(format!(
            "{}: grew past {} MiB",
            path.display(),
            cap / (1024 * 1024)
        ));
    }
    Ok(bytes)
}

/// A folder that may not exist yet as it will be: its nearest existing parent made canonical, the rest added.
fn canonical_to_be(p: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = p;
    loop {
        if let Ok(c) = std::fs::canonicalize(at) {
            return rest
                .iter()
                .rev()
                .fold(c, |acc: PathBuf, part| acc.join(part));
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

/// What a refresh did.
#[derive(Debug, Default)]
pub struct Refreshed {
    /// Level folders looked at.
    pub levels: usize,
    /// Found in the catalogue (level.txt written from it), and of those, level.txt changed.
    pub catalogued: usize,
    pub rewritten: usize,
    /// Kept for a readme (in the level's folder or beside its source) alone.
    pub readme_only: usize,
    /// Kept because their source could not be looked at for a readme (no `source =`, no source folder given, gone).
    pub unverified: Vec<String>,
    /// Moved into the unsorted folder: (name there, why).
    pub set_aside: Vec<(String, Reason)>,
    /// Links and other entries left alone.
    pub left: Vec<String>,
    pub failed: Vec<String>,
}

/// Brings a folder of levels already imported (`levels/community`) up to the catalogue, in place: each level's
/// level.txt gets the catalogue's title, Chinese title, author and difficulty (its other lines, `source =` among them,
/// kept); a folder that is not one level, or whose map is neither in the catalogue nor has a readme (in its folder or
/// beside its source in `source_root`, the collection its `source =` line names a path in), is moved into `unsorted`.
/// Nothing is deleted; only level.txt files are written.
pub fn refresh(
    dir: &Path,
    catalogue: &Catalogue,
    source_root: Option<&Path>,
    unsorted: &Path,
    tell: &mut dyn FnMut(String),
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Refreshed, String> {
    let meta = std::fs::symlink_metadata(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(format!(
            "{}: not a folder (a link is not followed)",
            dir.display()
        ));
    }
    let levels = canonical(dir)?;
    let aside = canonical_to_be(unsorted);
    if aside.starts_with(&levels) || levels.starts_with(&aside) {
        return Err(format!(
            "{}: the unsorted folder must be beside, not inside, {}",
            unsorted.display(),
            dir.display()
        ));
    }
    let source_root = source_root.map(canonical).transpose()?;
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(&levels)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .take(ENTRIES_MAX)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut r = Refreshed::default();
    let mut u = Unsorted::new(unsorted);
    for e in entries {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = u.finish();
            return Err(format!("stopped after {} levels", r.levels));
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let shown_name = printable(&name);
        let Ok(kind) = e.file_type() else { continue };
        if name.starts_with('.') || kind.is_file() {
            continue;
        }
        if !kind.is_dir() {
            r.left.push(format!("{name}: a link, left alone"));
            continue;
        }
        let path = e.path();
        r.levels += 1;
        let look = look_at_level(&path);
        // only a folder the toolbox made (its level.txt names a source) is ever moved: a folder given by mistake
        // (a download, a home folder) loses nothing
        let made =
            gamedata::package::read_small(&path.join("level.txt"), gamedata::package::INFO_MAX)
                .is_some_and(|b| source_of(&String::from_utf8_lossy(&b)).is_some());
        if !made {
            r.left.push(format!("{shown_name}: no level.txt with a source = line (not made by import-maps), left alone"));
            continue;
        }
        if !look.one_level {
            match u.move_dir(&path, Reason::NotOneLevel) {
                Ok(n) => tell(format!(
                    "set aside (not a single level): {shown_name} -> {}",
                    printable(&n)
                )),
                Err(e) => r.failed.push(e),
            }
            continue;
        }
        let info_path = path.join("level.txt");
        let info = gamedata::package::read_small(&info_path, gamedata::package::INFO_MAX);
        let text = info
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_default()
            .into_owned();
        let source = source_of(&text);
        let source_file = source
            .as_deref()
            .and_then(safe_rel)
            .zip(source_root.as_deref())
            .map(|(rel, root)| root.join(rel));
        let mut names: Vec<String> = Vec::new();
        if let Some(rel) = source.as_deref().and_then(safe_rel) {
            names.extend(rel.file_stem().map(|n| n.to_string_lossy().into_owned()));
        }
        names.push(name.clone());
        // as an import: the source's folder's name when that folder holds this one map
        if let (Some(file), Some(root)) = (&source_file, &source_root)
            && let Some(dir) = file.parent().filter(|d| d != root)
            && maps_in(dir) == 1
        {
            names.extend(dir.file_name().map(|n| n.to_string_lossy().into_owned()));
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(found) = catalogue.find(&refs) {
            r.catalogued += 1;
            if info.is_some() || std::fs::symlink_metadata(&info_path).is_err() {
                let new = merged_info(&text, &found.lines());
                if new != text {
                    match replace_file(&path, "level.txt", new.as_bytes()) {
                        Ok(()) => {
                            r.rewritten += 1;
                            tell(format!("{shown_name}: {}", printable(found.title())));
                        }
                        Err(e) => r.failed.push(e),
                    }
                }
            } else {
                r.failed.push(format!(
                    "{}: not a plain file of at most 4 KiB, left as it is",
                    info_path.display()
                ));
            }
            continue;
        }
        if look.readme || source_file.as_deref().is_some_and(has_readme) {
            r.readme_only += 1;
            continue;
        }
        let checked = source_file.as_deref().is_some_and(|f| {
            std::fs::symlink_metadata(f).is_ok_and(|m| m.is_file())
                && f.parent().is_some_and(|d| std::fs::read_dir(d).is_ok())
        });
        if !checked {
            tell(format!(
                "kept (its source could not be looked at for a readme): {shown_name}"
            ));
            r.unverified.push(name);
            continue;
        }
        match u.move_dir(&path, Reason::Uncatalogued) {
            Ok(n) => tell(format!(
                "set aside (not in the catalogue, no readme): {shown_name} -> {}",
                printable(&n)
            )),
            Err(e) => r.failed.push(e),
        }
    }
    if let Err(e) = u.finish() {
        r.failed.push(e);
    }
    r.set_aside = u.items().to_vec();
    Ok(r)
}

/// One line saying what a refresh did.
pub fn refresh_line(r: &Refreshed, unsorted: &Path) -> String {
    let count = |want: Reason| r.set_aside.iter().filter(|(_, w)| *w == want).count();
    format!(
        "{} levels: {} in the catalogue ({} level.txt written), {} kept for a readme, {} kept unverified; set aside into {}: {} not in the catalogue and without a readme, {} not a single level; {} left alone, {} failed",
        r.levels,
        r.catalogued,
        r.rewritten,
        r.readme_only,
        r.unverified.len(),
        shown(unsorted),
        count(Reason::Uncatalogued),
        count(Reason::NotOneLevel),
        r.left.len(),
        r.failed.len()
    )
}

/// What a level folder holds: whether it is one level as the game lists it (one `.nmo`, or the open format's
/// level.gltf, not both), and whether a readme lies in it.
struct Look {
    one_level: bool,
    readme: bool,
}

fn look_at_level(dir: &Path) -> Look {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Look {
            one_level: false,
            readme: false,
        };
    };
    let (mut nmo, mut gltf, mut readme) = (0usize, false, false);
    for e in read.flatten().take(4096) {
        if !e.file_type().is_ok_and(|k| k.is_file()) {
            continue;
        }
        let n = e.file_name().to_string_lossy().into_owned();
        if has_ext(Path::new(&n), &["nmo"]) {
            nmo += 1;
        } else if n.eq_ignore_ascii_case("level.gltf") {
            gltf = true;
        } else if is_readme(&n, "") {
            readme = true;
        }
    }
    Look {
        one_level: (nmo == 1) != gltf && nmo <= 1,
        readme,
    }
}

/// The value of level.txt's first `source =` line.
fn source_of(text: &str) -> Option<String> {
    text.lines().take(64).find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim().eq_ignore_ascii_case("source"))
            .then(|| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    })
}

/// A relative path of plain names only (no root, no "..", no "."), as a `source =` line may name inside the
/// collection; None for anything else.
fn safe_rel(s: &str) -> Option<PathBuf> {
    let p = Path::new(s);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::Normal(n) => out.push(n),
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// level.txt with the catalogue's `lines`: its leading comments (or the toolbox's), the catalogue's lines, then its
/// other lines but those the catalogue gives, within the game's 64 lines and 4 KiB.
fn merged_info(old: &str, lines: &[String]) -> String {
    let old = old.strip_prefix('\u{feff}').unwrap_or(old);
    let mut head: Vec<&str> = old
        .lines()
        .take_while(|l| l.trim_start().starts_with('#'))
        .collect();
    if head.is_empty() {
        head.push(INFO_HEADER);
    }
    let key_of = |l: &str| {
        l.split_once('=')
            .map(|(k, _)| k.trim().to_ascii_lowercase())
    };
    // a key the catalogue gives replaces the old line; one it has nothing for keeps the player's
    let given: Vec<String> = lines.iter().filter_map(|l| key_of(l)).collect();
    let rest: Vec<&str> = old
        .lines()
        .skip(head.len())
        .filter(|l| !l.trim().is_empty() && !key_of(l).is_some_and(|k| given.contains(&k)))
        .collect();
    // the source line first among them, so a cut at the game's caps never loses it
    let (source, others): (Vec<&str>, Vec<&str>) = rest
        .into_iter()
        .partition(|l| key_of(l).is_some_and(|k| k == "source"));
    let mut out = String::new();
    let all = head
        .into_iter()
        .map(str::to_owned)
        .chain(lines.iter().cloned())
        .chain(source.into_iter().map(str::to_owned))
        .chain(others.into_iter().map(str::to_owned));
    for l in all.take(64) {
        let len = out.len().saturating_add(l.len()).saturating_add(1);
        if len > usize::try_from(gamedata::package::INFO_MAX).unwrap_or(4096) {
            break;
        }
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// Replaces `dir/name` with `bytes`: written beside as a hidden part file and renamed over it (a link there is
/// replaced, never written through).
fn replace_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let tmp = dir.join(format!(".{name}.part-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    write_new(&tmp, bytes)?;
    let dst = dir.join(name);
    std::fs::rename(&tmp, &dst).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", dst.display())
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    static GO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// Sorting without a catalogue, setting aside beside `dst`.
    fn plain(dst: &Path) -> Sorting<'static> {
        let name = dst
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Sorting {
            catalogue: None,
            unsorted: dst.with_file_name(format!("{name}-unsorted")),
        }
    }

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
        assert_eq!(folder_name("nul.txt"), "nul_.txt");
        assert_eq!(folder_name("Aux.Level.nmo"), "Aux_.Level.nmo");
        assert_eq!(folder_name("com1 .x"), "com1 _.x");
        let long = format!("CON.{}", "x".repeat(60));
        let capped = folder_name(&long);
        assert!(
            capped.starts_with("CON_.") && capped.chars().count() <= NAME_MAX,
            "{capped}"
        );
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
        assert_eq!(
            unique(long, &mut taken),
            "Ballance Skyscraper (3D Maze; Hard…"
        );
        assert_eq!(
            unique(long, &mut taken),
            "Ballance Skyscraper (3D Maze; Hard… (2)"
        );
    }

    #[test]
    fn printed_names_cannot_drive_the_terminal() {
        assert_eq!(
            printable("a\u{1b}[2Jb\u{7}\u{202e}"),
            "a\\u{1b}[2Jb\\u{7}\\u{202e}"
        );
        assert_eq!(shown(Path::new("ok/name")), "ok/name");
    }

    #[test]
    fn a_name_the_disk_has_in_another_form_takes_the_next_number() {
        let dst = temp("fold");
        let src = temp("fold-src");
        std::fs::write(src.join("Map.nmo"), b"m").unwrap();
        // the disk has "Map" though `taken` does not know it (as a case-insensitive or normalising system would)
        std::fs::create_dir_all(dst.join("Map")).unwrap();
        let mut taken = HashSet::new();
        assert!(matches!(
            write_level(
                &dst,
                "Map",
                "Map",
                Path::new("Map.nmo"),
                b"m",
                &src.join("Map.nmo"),
                &[]
            ),
            Err(Written::Exists)
        ));
        let name = write_named(
            &dst,
            "Map",
            &mut taken,
            Path::new("Map.nmo"),
            b"m",
            &src.join("Map.nmo"),
            &[],
        )
        .unwrap();
        assert_eq!(name, "Map (2)");
        assert!(dst.join("Map (2)/Map.nmo").is_file());
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(&src);
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_folder_is_skipped_and_reported() {
        use std::os::unix::fs::PermissionsExt;
        let src = temp("unread");
        let dst = temp("unread-dst");
        std::fs::create_dir_all(src.join("locked")).unwrap();
        std::fs::write(src.join("locked/x.nmo"), b"x").unwrap();
        std::fs::write(src.join("open.nmo"), b"o").unwrap();
        std::fs::set_permissions(src.join("locked"), std::fs::Permissions::from_mode(0o000))
            .unwrap();
        let readable = std::fs::read_dir(src.join("locked")).is_ok(); // root reads it anyway
        let s = import(&src, &dst, &plain(&dst), &mut |_| {}, &GO);
        std::fs::set_permissions(src.join("locked"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let s = s.unwrap();
        if !readable {
            assert_eq!((s.imported, s.failed.len()), (1, 1));
        }
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
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
        assert_eq!(
            title_for(Path::new("/r/S1/W天空峡谷.Level.NMO"), root),
            "W天空峡谷"
        );
        assert_eq!(
            title_for(Path::new("/r/Custom Maps/Hello Mars.NMO"), root),
            "Hello Mars"
        );
        assert_eq!(
            title_for(Path::new("/r/反向Level_01.NMO"), root),
            "反向Level_01"
        );
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
        mk("Pro Racing/Level_01.NMO", b"r1");
        mk("Pro Racing/Level_02.NMO", b"r2");
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
        let s = import(&src, &dst, &plain(&dst), &mut |l| lines.push(l), &GO).unwrap();
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
                "Pro Racing - Level 01",
                "Pro Racing - Level 02",
                "TrackW2",
                "hello mars (2)"
            ]
        );
        assert_eq!(
            std::fs::read(dst.join("Hello Mars/preview.jpg")).unwrap(),
            b"jpg"
        );
        assert_eq!(
            std::fs::read(dst.join("TrackW2/preview.jpg")).unwrap(),
            b"tj"
        );
        assert_eq!(
            std::fs::read(dst.join("Hello Mars/Hello Mars.NMO")).unwrap(),
            b"mars"
        );
        let info = std::fs::read_to_string(dst.join("hello mars (2)/level.txt")).unwrap();
        assert!(
            !info.contains("title") && info.contains("source = Other"),
            "{info}"
        );
        // the source is untouched, and a second run makes new names beside the first
        assert!(src.join("Custom Maps/Hello Mars.NMO").exists());
        let again = import(&src, &dst, &plain(&dst), &mut |_| {}, &GO).unwrap();
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
    fn modified_originals_go_to_levels_modified_by_set() {
        let base = temp("modified");
        let src = base.join("download");
        let dst = base.join("levels/community");
        let mk = |p: &str, b: &[u8]| {
            let p = src.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        mk("45-Degree Rotated Maps/Level_01.NMO", b"r1");
        mk("45-Degree Rotated Maps/Level_01.jpg", b"pic");
        mk("Reverse Maps/反向Level_02(新).NMO", b"v2");
        mk("INVISIBLE maps/Level_03.nmo", b"i3");
        mk("Pro Racing/Season 1/Track.NMO", b"pro");
        let mut lines = Vec::new();
        let s = import(&src, &dst, &plain(&dst), &mut |l| lines.push(l), &GO).unwrap();
        assert_eq!((s.imported, s.modified), (1, 3), "{lines:?}");
        let m = base.join("levels/modified");
        assert_eq!(
            std::fs::read(m.join("45-Degree Rotated Maps/Level_01/Level_01.nmo")).unwrap(),
            b"r1"
        );
        assert_eq!(
            std::fs::read(m.join("45-Degree Rotated Maps/Level_01/preview.jpg")).unwrap(),
            b"pic"
        );
        assert_eq!(
            std::fs::read(m.join("Reverse Maps/反向Level_02(新)/反向Level_02(新).nmo")).unwrap(),
            b"v2"
        );
        assert!(m.join("INVISIBLE maps/Level_03/Level_03.nmo").is_file());
        // Pro Racing stays a community map
        assert!(dst.join("Track/Track.NMO").is_file(), "{lines:?}");
        // the game reads the sets by the same names
        assert!(modified_set(&src.join("Pro Racing/Season 1/Track.NMO"), &src).is_none());
        // a second run adds nothing
        let again = import(&src, &dst, &plain(&dst), &mut |_| {}, &GO).unwrap();
        assert_eq!((again.imported, again.modified), (0, 0));
        assert!(!m.join("Reverse Maps/反向Level_02(新) (2)").exists());
    }

    #[test]
    fn nested_folders_are_refused() {
        let src = temp("nest");
        assert!(
            import(
                &src,
                &src.join("out"),
                &plain(&src.join("out")),
                &mut |_| {},
                &GO
            )
            .is_err()
        );
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
        let s = import(&src, &dst, &plain(&dst), &mut |_| {}, &GO).unwrap();
        assert_eq!((s.imported, s.links.len()), (1, 2));
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(&outside);
    }

    const TABLE: &str = "| Posted | IDs | Original Name | English Name | Author | Difficulty | Type | Videos |\n\
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|\n\
| 2009 | 1 | 天马行空 | Pie in the Sky | 失衡技师 | 3 | C |  |\n\
| 2020 | 2 | Hello Mars | Hello Mars | someone | 2.5–3 | C |  |\n\
| 2022 | 3 | 璀璨星河 | Bright Galaxy | 其乐融融 |  | C |  |\n";

    #[test]
    fn readmes_are_told() {
        assert!(is_readme("README.txt", "x"));
        assert!(is_readme("notes.TXT", "x"));
        assert!(is_readme("地图说明.doc", "x"));
        assert!(is_readme("Map.pdf", "map"));
        assert!(is_readme("ReadMe.docx", "x"));
        assert!(!is_readme("level.txt", "x"));
        assert!(!is_readme("评分标准.doc", "x"));
        assert!(!is_readme("readme", "x"));
        assert!(!is_readme("readme.exe", "x"));
        assert!(!is_readme(".txt", "x"));
        assert!(!is_readme("Map.jpg", "map"));
    }

    #[test]
    fn with_a_catalogue_what_does_not_fit_is_set_aside() {
        let src = temp("cat-src");
        let base = temp("cat-base");
        let dst = base.join("community");
        let aside = base.join("community-unsorted");
        let mk = |p: &str, b: &[u8]| {
            let p = src.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        mk("Custom Maps/Pie in the Sky.NMO", b"pie");
        mk("Custom Maps/Bright Galaxy v1.2.NMO", b"bright");
        mk("Custom Maps/Nobody Knows.NMO", b"nobody");
        mk("Custom Maps/Nobody Knows.jpg", b"jpg");
        mk("Custom Maps/Packed.zip", b"zip");
        mk("Season/天马行空/W3.NMO", b"w3");
        mk("Readme Map/Mine.NMO", b"mine");
        mk("Readme Map/README.txt", b"hi");
        let c = crate::catalogue::parse(TABLE);
        let sorting = Sorting {
            catalogue: Some(&c),
            unsorted: aside.clone(),
        };
        let mut lines = Vec::new();
        let s = import(&src, &dst, &sorting, &mut |l| lines.push(l), &GO).unwrap();
        assert_eq!(
            (s.imported, s.catalogued, s.readme_only),
            (4, 3, 1),
            "{lines:?}"
        );
        let pie = std::fs::read_to_string(dst.join("Pie in the Sky/level.txt")).unwrap();
        assert_eq!(
            pie,
            "# Shown in the game's list of levels; edit as you like.\ntitle = Pie in the Sky\ntitle.zh = 天马行空\n\
             author = 失衡技师\ndifficulty = 6\nsource = Custom Maps/Pie in the Sky.NMO\n"
        );
        // named after the catalogue's title; matched by the version-less name and by its folder's name
        let bright = std::fs::read_to_string(dst.join("Bright Galaxy/level.txt")).unwrap();
        assert!(
            bright.contains("title.zh = 璀璨星河\n") && !bright.contains("difficulty"),
            "{bright}"
        );
        assert!(dst.join("Pie in the Sky (2)/W3.NMO").exists());
        let mine = std::fs::read_to_string(dst.join("Mine/level.txt")).unwrap();
        assert!(
            !mine.contains("title") && !mine.contains("difficulty"),
            "{mine}"
        );
        // copied aside, keeping the source's names; the source untouched
        assert_eq!(
            std::fs::read(aside.join("Custom Maps/Nobody Knows.NMO")).unwrap(),
            b"nobody"
        );
        assert_eq!(
            std::fs::read(aside.join("Custom Maps/Packed.zip")).unwrap(),
            b"zip"
        );
        assert!(
            src.join("Custom Maps/Nobody Knows.NMO").exists()
                && src.join("Custom Maps/Packed.zip").exists()
        );
        assert!(!dst.join("Nobody Knows").exists());
        let readme = std::fs::read_to_string(aside.join("README.txt")).unwrap();
        assert!(
            readme.contains("Custom Maps/Nobody Knows.NMO  —  not in the catalogue"),
            "{readme}"
        );
        assert!(
            readme.contains("Custom Maps/Packed.zip  —  packed archive"),
            "{readme}"
        );
        assert_eq!(s.set_aside.len(), 2);
        // without a catalogue every map is imported; archives are still set aside
        let base2 = temp("cat-base2");
        let s = import(
            &src,
            &base2.join("c"),
            &plain(&base2.join("c")),
            &mut |_| {},
            &GO,
        )
        .unwrap();
        assert_eq!((s.imported, s.set_aside.len()), (5, 1));
        // the unsorted folder inside the levels or the source is refused
        let inside = Sorting {
            catalogue: None,
            unsorted: dst.join("u"),
        };
        assert!(import(&src, &dst, &inside, &mut |_| {}, &GO).is_err());
        let inside = Sorting {
            catalogue: None,
            unsorted: src.join("u"),
        };
        assert!(import(&src, &dst, &inside, &mut |_| {}, &GO).is_err());
        assert!(!src.join("u").exists());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&base2);
    }

    #[test]
    fn a_refresh_rewrites_level_txt_and_moves_what_does_not_fit() {
        let src = temp("ref-src");
        let base = temp("ref-base");
        let levels = base.join("community");
        let aside = base.join("community-unsorted");
        let mk = |root: &Path, p: &str, b: &[u8]| {
            let p = root.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        let header = "# Shown in the game's list of levels; edit as you like.\n";
        mk(&src, "Custom Maps/Pie in the Sky.NMO", b"pie");
        mk(&src, "Custom Maps/Nobody.NMO", b"n");
        mk(&src, "Docs/Mine.NMO", b"m");
        mk(&src, "Docs/readme.txt", b"r");
        mk(&src, "Season/天马行空/W3.NMO", b"w");
        mk(&levels, "Pie in the Sky/Pie in the Sky.NMO", b"pie");
        mk(
            &levels,
            "Pie in the Sky/level.txt",
            format!("{header}title = Old\nversion = 2\nsource = Custom Maps/Pie in the Sky.NMO\n")
                .as_bytes(),
        );
        mk(&levels, "W3/W3.NMO", b"w");
        mk(
            &levels,
            "W3/level.txt",
            format!("{header}source = Season/天马行空/W3.NMO\n").as_bytes(),
        );
        mk(&levels, "Nobody/Nobody.NMO", b"n");
        mk(
            &levels,
            "Nobody/level.txt",
            format!("{header}source = Custom Maps/Nobody.NMO\n").as_bytes(),
        );
        mk(&levels, "Mine/Mine.NMO", b"m");
        mk(
            &levels,
            "Mine/level.txt",
            format!("{header}source = Docs/Mine.NMO\n").as_bytes(),
        );
        mk(&levels, "Own Readme/x.nmo", b"x");
        mk(&levels, "Own Readme/README.txt", b"r");
        mk(
            &levels,
            "Own Readme/level.txt",
            format!("{header}source = Elsewhere/x.nmo\n").as_bytes(),
        );
        mk(&levels, "Lost/Lost.NMO", b"l");
        mk(
            &levels,
            "Lost/level.txt",
            format!("{header}source = Custom Maps/Gone.NMO\n").as_bytes(),
        );
        mk(&levels, "Escape/E.NMO", b"e");
        mk(
            &levels,
            "Escape/level.txt",
            format!("{header}source = ../../etc/passwd\n").as_bytes(),
        );
        mk(&levels, "Two/a.nmo", b"a");
        mk(&levels, "Two/b.nmo", b"b");
        mk(
            &levels,
            "Two/level.txt",
            format!("{header}source = Two/a.nmo\n").as_bytes(),
        );
        mk(&levels, "Not Mine/a.nmo", b"a");
        mk(&levels, "Not Mine/b.nmo", b"b");
        std::fs::create_dir_all(levels.join("Empty")).unwrap();
        mk(&levels, "README.txt", b"the game's");
        let c = crate::catalogue::parse(TABLE);
        let mut lines = Vec::new();
        let r = refresh(&levels, &c, Some(&src), &aside, &mut |l| lines.push(l), &GO).unwrap();
        assert_eq!(
            (
                r.levels,
                r.catalogued,
                r.rewritten,
                r.readme_only,
                r.unverified.len(),
                r.set_aside.len()
            ),
            (10, 2, 2, 2, 2, 2),
            "{lines:?} {r:?}"
        );
        assert_eq!(
            std::fs::read_to_string(levels.join("Pie in the Sky/level.txt")).unwrap(),
            format!(
                "{header}title = Pie in the Sky\ntitle.zh = 天马行空\nauthor = 失衡技师\ndifficulty = 6\n\
                 source = Custom Maps/Pie in the Sky.NMO\nversion = 2\n"
            )
        );
        assert!(
            std::fs::read_to_string(levels.join("W3/level.txt"))
                .unwrap()
                .contains("title = Pie in the Sky\n")
        );
        // moved, never deleted; the maps kept where they were
        assert_eq!(
            std::fs::read(aside.join("Nobody/Nobody.NMO")).unwrap(),
            b"n"
        );
        assert!(aside.join("Two/a.nmo").exists());
        // folders the toolbox did not make are left alone, whatever they hold
        assert!(levels.join("Empty").is_dir() && levels.join("Not Mine/a.nmo").exists());
        assert_eq!(r.left.len(), 2, "{:?}", r.left);
        for kept in [
            "Mine",
            "Own Readme",
            "Lost",
            "Escape",
            "Pie in the Sky",
            "W3",
        ] {
            assert!(levels.join(kept).is_dir(), "{kept}");
        }
        assert!(levels.join("README.txt").is_file() && src.join("Custom Maps/Nobody.NMO").exists());
        // a second run changes nothing
        let again = refresh(&levels, &c, Some(&src), &aside, &mut |_| {}, &GO).unwrap();
        assert_eq!((again.rewritten, again.set_aside.len()), (0, 0));
        let readme = std::fs::read_to_string(aside.join("README.txt")).unwrap();
        assert_eq!(readme.matches("\n- ").count(), 2, "{readme}");
        // without the source collection nothing can be checked for a readme: nothing more is moved
        assert!(
            refresh(&levels, &c, None, &aside, &mut |_| {}, &GO)
                .unwrap()
                .set_aside
                .is_empty()
        );
        // the unsorted folder inside the levels is refused
        assert!(refresh(&levels, &c, Some(&src), &levels.join("u"), &mut |_| {}, &GO).is_err());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn merged_level_txt_stays_in_the_game_s_caps() {
        let old = format!(
            "# c\n# d\ntitle = x\nAuthor = y\n{}source = s\n",
            "k = v\n".repeat(200)
        );
        let m = merged_info(&old, &["title = T".into(), "difficulty = 3".into()]);
        // the player's author kept (the catalogue gives none here); the source kept ahead of the cut
        assert!(
            m.starts_with("# c\n# d\ntitle = T\ndifficulty = 3\nsource = s\nAuthor = y\n"),
            "{m}"
        );
        assert!(m.lines().count() <= 64 && m.len() <= 4096);
        assert_eq!(
            merged_info("", &["title = T".into()]),
            format!("{INFO_HEADER}\ntitle = T\n")
        );
        assert_eq!(safe_rel("a/b.nmo"), Some(PathBuf::from("a/b.nmo")));
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "./a"] {
            assert_eq!(safe_rel(bad), None, "{bad}");
        }
        assert_eq!(
            source_of("# x\nSource = A/B.NMO\nsource = other\n").as_deref(),
            Some("A/B.NMO")
        );
    }
}
