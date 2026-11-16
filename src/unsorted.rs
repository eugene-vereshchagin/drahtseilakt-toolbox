// The "unsorted" folder of `import-maps`: what does not fit the game's levels (packed archives, folders that are not
// one level, maps neither in the catalogue nor with a readme) is set aside there for the player to look at by hand,
// with a README.txt saying what each item is and why it is there. From a download the items are copied (the
// download is only read); a level folder already in the game's folders is moved (renamed on the same disk, never
// deleted). Nothing is written through a link, nothing already there is overwritten.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The largest file copied (a packed archive of a whole collection may be large).
const COPY_MAX: u64 = 4 * 1024 * 1024 * 1024;
/// The largest README.txt read back, and how many items it keeps.
const README_MAX: u64 = 1024 * 1024;
const ITEMS_MAX: usize = 20_000;
/// The separator of an item's path and its reason in README.txt.
const SEP: &str = "  —  ";
const HEADER: &str = "Set aside by drahtseilakt-toolbox import-maps: these did not fit the game's levels and wait to be looked at by hand.\n\
Packed archives: unpack them and import the folder. Folders that are not one level: split them into one folder per map.\n\
Maps neither in the catalogue nor with a readme: put a level folder of your choice into levels/user-levels yourself.\n\n";

/// Why an item was set aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reason {
    Archive,
    NotOneLevel,
    Uncatalogued,
}

impl Reason {
    pub fn text(self) -> &'static str {
        match self {
            Reason::Archive => "packed archive (unpack it and import the folder)",
            Reason::NotOneLevel => "not a single level (no map, or several)",
            Reason::Uncatalogued => "not in the catalogue and no readme beside it",
        }
    }
}

pub struct Unsorted {
    root: PathBuf,
    /// Made (and checked) on the first item: nothing is made when nothing is set aside.
    ready: bool,
    /// (path inside the folder, why) of this run.
    items: Vec<(String, Reason)>,
}

impl Unsorted {
    pub fn new(root: &Path) -> Self {
        Unsorted {
            root: root.to_path_buf(),
            ready: false,
            items: Vec::new(),
        }
    }

    pub fn items(&self) -> &[(String, Reason)] {
        &self.items
    }

    fn ready(&mut self) -> Result<(), String> {
        if !self.ready {
            std::fs::create_dir_all(&self.root)
                .map_err(|e| format!("{}: {e}", self.root.display()))?;
            plain_dir(&self.root)?;
            self.ready = true;
        }
        Ok(())
    }

    /// Copies `file` (inside `src_root`) to the same relative path here; one already there is left as it is.
    pub fn copy(&mut self, src_root: &Path, file: &Path, reason: Reason) -> Result<(), String> {
        let rel = file
            .strip_prefix(src_root)
            .map_err(|_| format!("{}: outside the source", file.display()))?;
        let parts: Vec<&std::ffi::OsStr> = rel
            .components()
            .map(|c| match c {
                std::path::Component::Normal(n) => Ok(n),
                _ => Err(format!("{}: not a plain path", rel.display())),
            })
            .collect::<Result<_, _>>()?;
        let Some((name, dirs)) = parts.split_last() else {
            return Err(format!("{}: no name", file.display()));
        };
        self.ready()?;
        let mut at = self.root.clone();
        for d in dirs {
            at.push(d);
            match std::fs::symlink_metadata(&at) {
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err(format!("{}: there and not a folder", at.display())),
                Err(_) => std::fs::create_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?,
            }
        }
        let dst = at.join(name);
        let shown = rel.to_string_lossy().into_owned();
        if std::fs::symlink_metadata(&dst).is_err() {
            copy_new(file, &dst)?;
        }
        self.items.push((shown, reason));
        Ok(())
    }

    /// Moves the folder `dir` here under its own name (or numbered: "Name (2)"), by renaming it on the same disk.
    pub fn move_dir(&mut self, dir: &Path, reason: Reason) -> Result<String, String> {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{}: no name", dir.display()))?;
        self.ready()?;
        for n in 1..=100u32 {
            let candidate = if n == 1 {
                name.clone()
            } else {
                format!("{name} ({n})")
            };
            let dst = self.root.join(&candidate);
            if std::fs::symlink_metadata(&dst).is_ok() {
                continue;
            }
            std::fs::rename(dir, &dst)
                .map_err(|e| format!("{} -> {}: {e}", dir.display(), dst.display()))?;
            self.items.push((candidate.clone(), reason));
            return Ok(candidate);
        }
        Err(format!("{}: no free name in {}", name, self.root.display()))
    }

    /// Writes README.txt: the items of earlier runs still there, and this run's.
    pub fn finish(&mut self) -> Result<(), String> {
        if self.items.is_empty() {
            return Ok(());
        }
        self.ready()?;
        let readme = self.root.join("README.txt");
        let mut lines: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let old = gamedata::package::read_small(&readme, README_MAX).unwrap_or_default();
        let old = String::from_utf8_lossy(&old);
        let new = self
            .items
            .iter()
            .map(|(p, r)| format!("- {}{SEP}{}", printable(p), r.text()));
        let earlier = old
            .lines()
            .filter(|l| l.starts_with("- "))
            .filter(|l| {
                // kept while the item is still there
                l.strip_prefix("- ")
                    .and_then(|l| l.rsplit_once(SEP))
                    .is_some_and(|(p, _)| std::fs::symlink_metadata(self.root.join(p)).is_ok())
            })
            .map(str::to_owned);
        for l in earlier.chain(new) {
            let path = l.rsplit_once(SEP).map_or(l.as_str(), |(p, _)| p).to_owned();
            if lines.len() < ITEMS_MAX && seen.insert(path) {
                lines.push(l);
            }
        }
        lines.sort();
        let text = format!("{HEADER}{}\n", lines.join("\n"));
        let tmp = self
            .root
            .join(format!(".README.txt.part-{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        write_new(&tmp, text.as_bytes())?;
        std::fs::rename(&tmp, &readme).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", readme.display())
        })
    }
}

/// A folder that is itself a folder, not a link to one.
fn plain_dir(p: &Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if m.file_type().is_symlink() || !m.is_dir() {
        return Err(format!(
            "{}: a link or not a folder; give the folder itself",
            p.display()
        ));
    }
    Ok(())
}

/// Copies the plain file `from` (never through a link) to the new file `to`, through a hidden part file renamed into
/// place when complete; at most COPY_MAX bytes.
fn copy_new(from: &Path, to: &Path) -> Result<(), String> {
    let fail = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let meta = std::fs::symlink_metadata(from).map_err(|e| fail(from, e))?;
    if !meta.is_file() {
        return Err(format!("{}: not a plain file", from.display()));
    }
    if meta.len() > COPY_MAX {
        return Err(format!("{}: larger than 4 GiB, not copied", from.display()));
    }
    let src = gamedata::package::open_plain(from, &meta).map_err(|e| fail(from, e))?;
    let dir = to
        .parent()
        .ok_or_else(|| format!("{}: no folder", to.display()))?;
    let tmp = dir.join(format!(
        ".part-{}-{}",
        std::process::id(),
        gamedata::fnv1a(to.as_os_str().as_encoded_bytes())
    ));
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| fail(&tmp, e))?;
    let result = (|| {
        let mut limited = std::io::Read::take(src, COPY_MAX.saturating_add(1));
        let n = std::io::copy(&mut limited, &mut out).map_err(|e| fail(from, e))?;
        if n > COPY_MAX {
            return Err(format!("{}: grew past 4 GiB", from.display()));
        }
        out.flush().map_err(|e| fail(&tmp, e))?;
        drop(out);
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(format!("{}: already there", to.display()));
        }
        std::fs::rename(&tmp, to).map_err(|e| fail(to, e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(bytes)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A path for README.txt: control and format characters escaped, so a line stays one line.
fn printable(s: &str) -> String {
    crate::import::printable(s)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("toolbox-unsorted-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn items_are_copied_moved_and_listed() {
        let src = temp("src");
        let base = temp("base");
        let root = base.join("community-unsorted");
        std::fs::create_dir_all(src.join("a/b")).unwrap();
        std::fs::write(src.join("a/b/pack.zip"), b"zip").unwrap();
        std::fs::create_dir_all(base.join("Lvl")).unwrap();
        std::fs::write(base.join("Lvl/x.nmo"), b"x").unwrap();
        let mut u = Unsorted::new(&root);
        // nothing set aside: nothing made
        u.finish().unwrap();
        assert!(!root.exists());
        u.copy(&src, &src.join("a/b/pack.zip"), Reason::Archive)
            .unwrap();
        assert_eq!(std::fs::read(root.join("a/b/pack.zip")).unwrap(), b"zip");
        assert!(src.join("a/b/pack.zip").exists(), "the source is only read");
        assert_eq!(
            u.move_dir(&base.join("Lvl"), Reason::Uncatalogued).unwrap(),
            "Lvl"
        );
        assert!(!base.join("Lvl").exists() && root.join("Lvl/x.nmo").exists());
        u.finish().unwrap();
        let readme = std::fs::read_to_string(root.join("README.txt")).unwrap();
        assert!(
            readme.contains("- a/b/pack.zip  —  packed archive"),
            "{readme}"
        );
        assert!(
            readme.contains("- Lvl  —  not in the catalogue"),
            "{readme}"
        );
        // a second run keeps the earlier items, numbers a second folder of one name, never overwrites
        std::fs::create_dir_all(base.join("Lvl")).unwrap();
        std::fs::write(src.join("a/b/pack.zip"), b"changed").unwrap();
        let mut u = Unsorted::new(&root);
        assert_eq!(
            u.move_dir(&base.join("Lvl"), Reason::NotOneLevel).unwrap(),
            "Lvl (2)"
        );
        u.copy(&src, &src.join("a/b/pack.zip"), Reason::Archive)
            .unwrap();
        assert_eq!(std::fs::read(root.join("a/b/pack.zip")).unwrap(), b"zip");
        u.finish().unwrap();
        let readme = std::fs::read_to_string(root.join("README.txt")).unwrap();
        assert_eq!(readme.matches("\n- ").count(), 3, "{readme}");
        // no part files left
        assert!(
            std::fs::read_dir(&root)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with('.'))
        );
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_written_through() {
        use std::os::unix::fs::symlink;
        let src = temp("lsrc");
        let base = temp("lbase");
        let outside = temp("lout");
        std::fs::create_dir_all(src.join("d")).unwrap();
        std::fs::write(src.join("d/x.zip"), b"x").unwrap();
        std::fs::create_dir_all(base.join("u")).unwrap();
        symlink(&outside, base.join("u/d")).unwrap();
        let mut u = Unsorted::new(&base.join("u"));
        assert!(u.copy(&src, &src.join("d/x.zip"), Reason::Archive).is_err());
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
        // the folder itself a link: refused
        symlink(&outside, base.join("linked")).unwrap();
        let mut u = Unsorted::new(&base.join("linked"));
        assert!(u.copy(&src, &src.join("d/x.zip"), Reason::Archive).is_err());
        // a linked source file: not copied
        symlink(src.join("d/x.zip"), src.join("d/y.zip")).unwrap();
        let mut u = Unsorted::new(&base.join("v"));
        assert!(u.copy(&src, &src.join("d/y.zip"), Reason::Archive).is_err());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
