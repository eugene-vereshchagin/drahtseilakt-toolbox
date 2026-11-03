// The community's catalogue of custom maps (CustomMapList.md of github.com/pseudoacetyl/BallanceCustomMaps, a markdown
// table: Posted | IDs | Original Name | English Name | Author | Difficulty | Type | Videos), read so `import-maps` can
// give each map its English title, its Chinese one, its author and its difficulty. The file is a stranger's download:
// its size, its rows and its cells are capped, a malformed row is skipped, and every value is cleaned as the game
// cleans what it shows.

use std::collections::HashMap;

/// The largest catalogue read, the rows and cells taken, the longest cell used, and the versions of one name.
const FILE_MAX: u64 = 4 * 1024 * 1024;
const ROWS_MAX: usize = 20_000;
const CELLS_MAX: usize = 32;
const CELL_MAX: usize = 1024;
const ALTS_MAX: usize = 8;

/// One map of the catalogue. A name cell may hold several versions ("A \| B"); `original[i]` and `english[i]` are the
/// same version when both cells hold as many.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub original: Vec<String>,
    pub english: Vec<String>,
    pub author: String,
    /// The game's 1–10: the catalogue's 1–5 (with halves) doubled; a range's midpoint.
    pub difficulty: Option<u8>,
}

/// A map matched: its entry and which of its versions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Found<'a> {
    pub entry: &'a Entry,
    pub version: usize,
}

#[derive(Debug, Default)]
pub struct Catalogue {
    pub entries: Vec<Entry>,
    /// Rows left out as malformed.
    pub skipped: usize,
    /// Normalised name -> (entry, version); `loose` the same without a trailing version number.
    exact: HashMap<String, Vec<(usize, usize)>>,
    loose: HashMap<String, Vec<(usize, usize)>>,
}

/// Reads the catalogue file (a plain file of at most FILE_MAX, never through a link).
pub fn read(path: &std::path::Path) -> Result<Catalogue, String> {
    let bytes = gamedata::package::read_small(path, FILE_MAX)
        .ok_or_else(|| format!("{}: not a plain readable file of at most 4 MiB", path.display()))?;
    let c = parse(&String::from_utf8_lossy(&bytes));
    if c.entries.is_empty() {
        return Err(format!("{}: no maps found (not the catalogue's table)", path.display()));
    }
    Ok(c)
}

/// The catalogue's table: the header row names the columns used; rows before it, rows with too few cells, overlong
/// cells or no name are skipped.
pub fn parse(text: &str) -> Catalogue {
    let mut c = Catalogue::default();
    let mut columns: Option<[usize; 4]> = None;
    for line in text.lines() {
        if c.entries.len().saturating_add(c.skipped) >= ROWS_MAX {
            break;
        }
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let Some(cells) = cells(line) else {
            c.skipped = c.skipped.saturating_add(1);
            continue;
        };
        let Some([o, e, a, d]) = columns else {
            columns = header(&cells);
            continue;
        };
        if cells
            .iter()
            .all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
        {
            continue;
        }
        let get = |i: usize| cells.get(i).map(String::as_str);
        let (Some(oc), Some(ec), Some(ac), Some(dc)) = (get(o), get(e), get(a), get(d)) else {
            c.skipped = c.skipped.saturating_add(1);
            continue;
        };
        let original = versions(oc);
        let english = versions(ec);
        if original.is_empty() && english.is_empty() {
            c.skipped = c.skipped.saturating_add(1);
            continue;
        }
        let author = clean(&versions(ac).join(" / "), gamedata::package::FIELD_MAX);
        c.entries.push(Entry {
            original,
            english,
            author,
            difficulty: difficulty(dc),
        });
    }
    c.index();
    c
}

/// A table row's cells: split at `|` not escaped as `\|`, unescaped; None when there are too many or one is too long.
fn cells(line: &str) -> Option<Vec<String>> {
    let inner = line.strip_prefix('|').unwrap_or(line);
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars();
    let mut len = 0usize;
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                // an escaped bar stays a bar inside the cell (a version separator)
                Some(n) if n.is_ascii_punctuation() => cell.push(n),
                Some(n) => {
                    cell.push('\\');
                    cell.push(n);
                }
                None => cell.push('\\'),
            },
            '|' => {
                out.push(std::mem::take(&mut cell));
                len = 0;
                if out.len() > CELLS_MAX {
                    return None;
                }
                continue;
            }
            _ => cell.push(ch),
        }
        len = len.saturating_add(1);
        if len > CELL_MAX {
            return None;
        }
    }
    if !cell.trim().is_empty() {
        out.push(cell);
    }
    Some(out.into_iter().map(|c| c.trim().to_owned()).collect())
}

/// The columns of Original Name, English Name, Author and Difficulty, if `cells` is the header row.
fn header(cells: &[String]) -> Option<[usize; 4]> {
    let at = |want: &str| cells.iter().position(|c| c.trim().eq_ignore_ascii_case(want));
    Some([
        at("Original Name")?,
        at("English Name")?,
        at("Author")?,
        at("Difficulty")?,
    ])
}

/// A name cell's versions ("A | B"), line breaks (`<br>`) as spaces, each cleaned, empty ones dropped.
fn versions(cell: &str) -> Vec<String> {
    let cell = cell.replace("<br />", " ").replace("<br/>", " ").replace("<br>", " ");
    cell.split('|')
        .map(|v| clean(v, CELL_MAX))
        .filter(|v| !v.is_empty())
        .take(ALTS_MAX)
        .collect()
}

/// The catalogue's difficulty ("3", "4.5", "2.5–3") on the game's scale of 1–10: doubled, a range's midpoint (rounded
/// up), clamped; None when it is missing or not a difficulty.
pub fn difficulty(cell: &str) -> Option<u8> {
    let parts: Vec<&str> = cell.split(['–', '—', '-', '~']).map(str::trim).collect();
    let halves = |s: &str| -> Option<u32> {
        let (whole, frac) = s.split_once('.').unwrap_or((s, "0"));
        if whole.is_empty() || whole.len() > 2 || !whole.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let w: u32 = whole.parse().ok()?;
        let f = match frac {
            "0" | "00" => 0,
            "5" | "50" => 1,
            _ => return None,
        };
        let h = w.checked_mul(2)?.checked_add(f)?;
        (h <= 10).then_some(h)
    };
    let h = match parts.as_slice() {
        [one] => halves(one)?,
        [a, b] => {
            let (a, b) = (halves(a)?, halves(b)?);
            a.checked_add(b)?.checked_add(1)? / 2
        }
        _ => return None,
    };
    u8::try_from(h.clamp(1, 10)).ok()
}

impl Catalogue {
    fn index(&mut self) {
        for (i, e) in self.entries.iter().enumerate() {
            for names in [&e.original, &e.english] {
                for (v, name) in names.iter().enumerate() {
                    add(&mut self.exact, key(name), (i, v));
                    // only a name without a version of its own: "X 1.0" is never taken for the catalogue's "X 2.0"
                    if loose_key(name) == key(name) {
                        add(&mut self.loose, key(name), (i, v));
                    }
                }
                // a file named after the whole cell ("A ｜ B.NMO")
                if names.len() > 1 {
                    add(&mut self.exact, key(&names.join("|")), (i, 0));
                }
            }
        }
    }

    /// The entry `names` (a map's file name, its folder's name...) stand for: the first name that matches exactly
    /// (normalised: case, punctuation, spaces, full-width forms, "v1.0"/"V1.0"/"1.0"), else the first that matches
    /// without its version number; a name matching several maps matches none.
    pub fn find(&self, names: &[&str]) -> Option<Found<'_>> {
        for (index, k) in [(&self.exact, key as fn(&str) -> String), (&self.loose, loose_key)] {
            for name in names {
                let k = k(name);
                // a very short name ("Go", "A", "7") says too little to be matched
                if k.chars().count() < 3 {
                    continue;
                }
                let Some(hits) = index.get(&k) else { continue };
                let first = hits.first()?;
                if hits.iter().all(|h| h.0 == first.0) {
                    let entry = self.entries.get(first.0)?;
                    return Some(Found {
                        entry,
                        version: first.1,
                    });
                }
            }
        }
        None
    }
}

fn add(map: &mut HashMap<String, Vec<(usize, usize)>>, key: String, at: (usize, usize)) {
    if !key.is_empty() {
        let hits = map.entry(key).or_default();
        if !hits.contains(&at) {
            hits.push(at);
        }
    }
}

impl Found<'_> {
    /// The English title of the version matched (the same version when the cells hold as many; else the first).
    pub fn title(&self) -> &str {
        let e = self.entry;
        e.english
            .get(self.version)
            .or_else(|| e.english.first())
            .or_else(|| e.original.get(self.version))
            .or_else(|| e.original.first())
            .map_or("", String::as_str)
    }

    /// The original title when it is another one and holds Chinese (or Japanese, Korean) characters.
    pub fn title_zh(&self) -> Option<&str> {
        let e = self.entry;
        let o = e.original.get(self.version).or_else(|| e.original.first())?;
        (o != self.title() && o.chars().any(is_cjk)).then_some(o.as_str())
    }

    /// The level.txt lines the catalogue gives: title, title.zh, author, difficulty (values cleaned, titles at most
    /// the game's 40 characters).
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let title = clean(self.title(), gamedata::package::TITLE_MAX);
        if !title.is_empty() {
            out.push(format!("title = {title}"));
        }
        if let Some(zh) = self.title_zh().map(|z| clean(z, gamedata::package::TITLE_MAX))
            && !zh.is_empty()
        {
            out.push(format!("title.zh = {zh}"));
        }
        let author = clean(&self.entry.author, gamedata::package::FIELD_MAX);
        if !author.is_empty() {
            out.push(format!("author = {author}"));
        }
        if let Some(d) = self.entry.difficulty {
            out.push(format!("difficulty = {d}"));
        }
        out
    }
}

pub fn is_cjk(c: char) -> bool {
    matches!(u32::from(c),
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xAC00..=0xD7AF | 0x20000..=0x3134F)
}

/// Full-width ASCII forms (Ａ, ｜, （) as their ASCII ones, the ideographic space as a space.
fn fold(c: char) -> char {
    match u32::from(c) {
        n @ 0xFF01..=0xFF5E => char::from_u32(n.saturating_sub(0xFEE0)).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    }
}

/// A name as matched: full-width folded, lower case, only letters, digits and `+` (so "Level++ 3" and "Level+++3"
/// stay apart), a `v` before a version number dropped ("V1.0", "v1.0" and "1.0" alike), a BML ".Level" end dropped.
pub fn key(name: &str) -> String {
    let lower: Vec<char> = name.chars().map(fold).flat_map(char::to_lowercase).collect();
    let lower: &[char] = match lower.len().checked_sub(6) {
        Some(at) if lower.get(at..).is_some_and(|t| t.iter().copied().eq(".level".chars())) => {
            lower.get(..at).unwrap_or(&lower)
        }
        _ => &lower,
    };
    let mut out = String::new();
    for (i, &c) in lower.iter().enumerate() {
        if !(c.is_alphanumeric() || c == '+') {
            continue;
        }
        let before = i.checked_sub(1).and_then(|p| lower.get(p));
        let after = lower.get(i.saturating_add(1));
        if c == 'v' && after.is_some_and(char::is_ascii_digit) && !before.is_some_and(char::is_ascii_alphabetic) {
            continue;
        }
        out.push(c);
    }
    out
}

/// `key` of the name without a trailing version ("Bright Galaxy v1.2" -> "brightgalaxy"); a number without a `v` or a
/// dot ("Level++ 9") is part of the name and stays.
pub fn loose_key(name: &str) -> String {
    let folded: String = name.chars().map(fold).collect();
    let t = folded.trim_end();
    let digits_start = t
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '.')
        .last()
        .map(|(i, _)| i);
    let Some(at) = digits_start else {
        return key(name);
    };
    let tail = t.get(at..).unwrap_or_default();
    if !tail.bytes().any(|b| b.is_ascii_digit()) {
        return key(name);
    }
    let head = t.get(..at).unwrap_or_default();
    let (head, v) = match head.strip_suffix(['v', 'V']) {
        Some(h) if !h.ends_with(|c: char| c.is_ascii_alphabetic()) => (h, true),
        _ => (head, false),
    };
    if !(v || tail.contains('.')) {
        return key(name);
    }
    let k = key(head);
    if k.is_empty() { key(name) } else { k }
}

/// Text without control, format and direction characters, spaces made one, trimmed, at most `max` characters.
pub fn clean(s: &str, max: usize) -> String {
    crate::import::clean(s, max)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TABLE: &str = "# Ballance Custom Map List\n\n\
| Posted | IDs | Original Name | English Name | Author | Difficulty | Type | Videos |\n\
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|\n\
| 2009-08-19 | 630462643 | 天马行空 | Pie in the Sky | 失衡技师 | 3 | Comprehensive |  |\n\
| (\\~2010-03-20) |  | 挑战250V1.0 | Challenge 250 v1.0 | 2jjy | 2 | Logical | [a](https://x/y)<br />[b](https://z) |\n\
| 2011-10-23 | 1 | 急速下坠（测试版） \\| 急速下坠V1.0 | Rapidly Plunging (Beta; v1) \\| Rapidly Plunging v1.0 | skb7757 | 3.5 | C |  |\n\
| 2013 |  | 急速滚球-跳跃世界 | Rapidly Rolling Ball—Jump World | 是♂不是♀ \\| 俊杰大兔子 | 2.5–3 | Racing | |\n\
| 2022 | 7 | 璀璨星河 | Bright Galaxy | 其乐融融 |  | Logical | |\n\
| 2026 | C108 | Level++ 9 | Level++ 9 | x | 4 | C | |\n\
| 2026 | C114 | Level+++3 | Level+++3 | x | 4 | C | |\n\
| 2026 | C115 | Level++ 3 | Level++ 3 | x | 4.5 | C | |\n\
| 2011 |  | Unknown | Unknown | 61粉丝 | 2 | C |  |\n\
| 2012 |  | Unknown | Unknown | other | 2 | C |  |\n\
| 2020 |  | Hello Mars | Hello Mars | someone | 1 | C |  |\n";

    #[test]
    fn the_table_is_read() {
        let c = parse(TABLE);
        assert_eq!(c.entries.len(), 11);
        let pie = &c.entries[0];
        assert_eq!(
            (
                pie.original[0].as_str(),
                pie.english[0].as_str(),
                pie.author.as_str(),
                pie.difficulty
            ),
            ("天马行空", "Pie in the Sky", "失衡技师", Some(6))
        );
        assert_eq!(c.entries[2].original.len(), 2);
        assert_eq!(c.entries[2].english[1], "Rapidly Plunging v1.0");
        assert_eq!(c.entries[3].author, "是♂不是♀ / 俊杰大兔子");
        assert_eq!(c.entries[4].difficulty, None);
    }

    #[test]
    fn difficulty_is_doubled() {
        assert_eq!(difficulty("1"), Some(2));
        assert_eq!(difficulty("5"), Some(10));
        assert_eq!(difficulty("3.5"), Some(7));
        assert_eq!(difficulty(" 4.5 "), Some(9));
        assert_eq!(difficulty("0.5"), Some(1));
        assert_eq!(difficulty("0"), Some(1));
        // a range: its midpoint, rounded up
        assert_eq!(difficulty("2.5–3"), Some(6));
        assert_eq!(difficulty("2-3"), Some(5));
        assert_eq!(difficulty("1~5"), Some(6));
        for bad in [
            "",
            "?",
            "6",
            "3.3",
            "-1",
            "1-2-3",
            "99999999999999",
            "½",
            "3.",
            ".5",
            "four",
            "1–",
        ] {
            assert_eq!(difficulty(bad), None, "{bad}");
        }
    }

    #[test]
    fn names_match_in_any_form() {
        let c = parse(TABLE);
        let title = |n: &str| c.find(&[n]).map(|f| f.title().to_owned());
        assert_eq!(title("Pie in the Sky").as_deref(), Some("Pie in the Sky"));
        assert_eq!(title("pie_in-the  SKY").as_deref(), Some("Pie in the Sky"));
        assert_eq!(title("ＰＩＥ　ｉｎ the Sky").as_deref(), Some("Pie in the Sky"));
        assert_eq!(title("天马行空").as_deref(), Some("Pie in the Sky"));
        assert_eq!(title("天马行空.Level").as_deref(), Some("Pie in the Sky"));
        assert_eq!(title("Challenge 250 V1.0").as_deref(), Some("Challenge 250 v1.0"));
        assert_eq!(title("Challenge 250 1.0").as_deref(), Some("Challenge 250 v1.0"));
        assert_eq!(title("挑战250v1.0").as_deref(), Some("Challenge 250 v1.0"));
        // a version number the catalogue lacks
        assert_eq!(title("Bright Galaxy v1.2").as_deref(), Some("Bright Galaxy"));
        // a number that is part of the name is no version
        assert_eq!(title("Level++ 9").as_deref(), Some("Level++ 9"));
        assert_eq!(title("Level+++3").as_deref(), Some("Level+++3"));
        assert_eq!(title("Level++ 3").as_deref(), Some("Level++ 3"));
        assert_eq!(title("Level++ 5"), None);
        // another version than the catalogue's is no match; a very short name matches nothing
        assert_eq!(title("Challenge 250 v3.0"), None);
        assert_eq!(title("Pie in the Sky v2"), Some("Pie in the Sky".to_owned()));
        // the versions of one cell, and a file named after the whole cell
        let f = c.find(&["急速下坠V1.0"]).unwrap();
        assert_eq!(
            (f.title(), f.title_zh()),
            ("Rapidly Plunging v1.0", Some("急速下坠V1.0"))
        );
        let f = c
            .find(&["Rapidly Plunging (Beta; v1) ｜ Rapidly Plunging v1.0"])
            .unwrap();
        assert_eq!(f.title(), "Rapidly Plunging (Beta; v1)");
        // two maps of one name: neither
        assert_eq!(title("Unknown"), None);
        // the first name that matches; a folder's name when the file's does not
        assert_eq!(
            c.find(&["Level_07", "Hello Mars"])
                .map(|f| f.title().to_owned())
                .as_deref(),
            Some("Hello Mars")
        );
        assert_eq!(title("Pie"), None);
        assert_eq!(title(""), None);
        assert_eq!(title("!!!"), None);
    }

    #[test]
    fn level_txt_lines() {
        let c = parse(TABLE);
        assert_eq!(
            c.find(&["Pie in the Sky"]).unwrap().lines(),
            [
                "title = Pie in the Sky",
                "title.zh = 天马行空",
                "author = 失衡技师",
                "difficulty = 6"
            ]
        );
        // no Chinese title when the original is the English one; no difficulty when unknown
        assert_eq!(
            c.find(&["Hello Mars"]).unwrap().lines(),
            ["title = Hello Mars", "author = someone", "difficulty = 2"]
        );
        assert_eq!(c.find(&["璀璨星河"]).unwrap().lines().len(), 3);
    }

    #[test]
    fn hostile_tables_are_survived() {
        // no header: nothing
        assert!(parse("| a | b |\n| 天马行空 | x |\n").entries.is_empty());
        assert!(parse("").entries.is_empty());
        assert!(parse("\u{0}\u{ffff}|||||\\").entries.is_empty());
        // short rows, overlong cells, too many cells, rows of bars: skipped
        let head = "| Original Name | English Name | Author | Difficulty |\n";
        let long = format!("{head}| {} | B | c | 1 |\n| x |\n| A | B | c | 1 |\n", "x".repeat(5000));
        let c = parse(&long);
        assert_eq!((c.entries.len(), c.skipped), (1, 2));
        let wide = format!("{head}|{}\n", " a |".repeat(100));
        assert_eq!(parse(&wide).entries.len(), 0);
        assert_eq!(parse(&format!("{head}| | | c | 1 |\n")).entries.len(), 0);
        // control and direction characters, line breaks and huge versions lists cleaned and capped
        let c = parse(&format!(
            "{head}| A\u{202e}\u{1b}[2J | B<br>C | evil\u{7}{} | 3 |\n",
            " \\| x".repeat(50)
        ));
        let e = &c.entries[0];
        assert_eq!((e.original[0].as_str(), e.english[0].as_str()), ("A[2J", "B C"));
        assert!(e.author.chars().count() <= gamedata::package::FIELD_MAX);
        assert!(!e.author.contains('\u{7}'));
        // escaped backslash at the end, a lone escape
        let c = parse(&format!("{head}| a\\\\ | b\\ | c | 2 |\n"));
        assert_eq!(c.entries.len(), 1);
        // very many rows: capped
        let many = format!("{head}{}", "| a | b | c | 1 |\n".repeat(ROWS_MAX + 10));
        assert_eq!(parse(&many).entries.len(), ROWS_MAX);
        // level.txt lines never break a line or exceed the game's caps
        let c = parse(&format!(
            "{head}| {}\n | {} | a\nb | 2 |\n",
            "长".repeat(100),
            "T".repeat(300)
        ));
        for e in &c.entries {
            for l in (Found { entry: e, version: 0 }).lines() {
                assert!(!l.contains('\n') && l.chars().count() <= 60, "{l}");
            }
        }
    }

    #[test]
    fn keys() {
        assert_eq!(key("Challenge 250 v1.0"), "challenge25010");
        assert_eq!(key("挑战250V1.0"), "挑战25010");
        assert_eq!(key("Revolver"), "revolver");
        assert_eq!(key("Lv1"), "lv1");
        assert_eq!(key("W天空峡谷.Level"), "w天空峡谷");
        assert_eq!(
            loose_key("AAAAAA Class Islands Scenery v1.0"),
            "aaaaaaclassislandsscenery"
        );
        assert_eq!(loose_key("Amusing Imitation 2.0"), "amusingimitation");
        assert_eq!(loose_key("Level++ 9"), "level++9");
        assert_eq!(loose_key("v1.0"), key("v1.0"));
        assert!(is_cjk('天') && !is_cjk('A'));
    }
}
