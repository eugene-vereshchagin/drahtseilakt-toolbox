# drahtseilakt-toolbox

**English** · [Русский](README.ru.md) · [Deutsch](README.de.md) · [简体中文](README.zh.md)

Prepares levels for [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), the new Ballance engine,
on the player's own machine and from the player's own copy of the game. The game only plays what is in its level
folders; everything heavy that makes levels look modern lives here, so the game needs none of it.

```
drahtseilakt-toolbox                     (the window)
drahtseilakt-toolbox remaster [--levels LIST] [--scale N] [--model NAME] [--relief S] [--data DIR]
drahtseilakt-toolbox sounds [--data DIR]
drahtseilakt-toolbox remaster-open DIR [--scale N] [--model NAME] [--relief S]
drahtseilakt-toolbox remaster-nmo FILE.nmo [--scale N] [--model NAME] [--relief S]
drahtseilakt-toolbox relief DIR [--scale N] [--relief S]
drahtseilakt-toolbox import-maps DIR [--to DIR] [--catalogue FILE] [--unsorted DIR]
drahtseilakt-toolbox import-maps --refresh DIR --catalogue FILE [--source DIR] [--unsorted DIR]
```

**Folders (2026-11-12).** The game's `levels/` holds `ballance-original/` (your copy), `ballance-remaster/` (what `remaster` writes) and `user-levels/` (what `import-maps` writes by default; the earlier names `original/`, `remastered/` and `downloaded/` are moved by the game at its first start). A level counts as a level only when it lies in its own named folder that holds either one `.nmo` file or an open-format level, plus the description file `level.txt`: `import-maps` writes exactly that.

`remaster-open` remasters one open-format level folder in place (rounder shapes, larger textures, masonry relief; the plain originals stay in `textures.original/`, so a second run grows nothing twice). `remaster-nmo` makes `FILE.remaster/` beside a community map: its looks and pictures, which the game puts on the map's meshes. The Back to 2004 mod shows both plain.

Started without a command it opens its window (Linux X11 or Wayland, Windows, macOS; it remembers the model and
scale chosen last, in `toolbox.txt` in the game's data folder): your copy of the game, the
scale, the Real-ESRGAN model, the relief, a Make button, the progress per level and part with Cancel, the result and
a texture before and after. Its texts are in English, Russian, German, Simplified Chinese, French, Spanish and Italian, from the system's
language or chosen in the window. The window looks like the game's own menu (a dusk sky, framed dark panels, pill
buttons with a ball, the game's Tektur lettering), drawn by the toolbox itself: it needs no copy and ships no
pictures. Its two sections are "Remaster the original levels" and "Import community maps", each with one main
button; the Real-ESRGAN model and the relief are folded under Advanced. No font is built into the
program: the window reads the game's own font files (Tektur, Noto Sans and, for Chinese, Noto Sans SC; SIL OFL, in the
game's `assets/fonts/` with their licences) at start, from where the game finds its assets (beside the program, the
portable folder, the game's tree when built from source), else from a `fonts/` folder beside the toolbox. Without
Noto Sans SC Chinese uses a font the system has (Noto CJK, Microsoft YaHei, PingFang...); without Tektur or Noto Sans
the toolbox says which file it missed and where it looked, and opens no window.

`remaster` converts the original levels the copy has into the game's `levels/ballance-remaster/` (the open format), gives
each level rounder shapes (written into the level beside the original meshes, which the physics keeps using, so runs
are the same) and makes its textures N times larger (default 4; 1 leaves them). With Real-ESRGAN the textures gain
detail, else they are resampled with Lanczos (as are textures under 64 pixels, where it would invent detail). Real-ESRGAN is looked for in `realesrgan/` beside the toolbox's
program, then on the PATH; the version used is pinned in [third-party.txt](third-party.txt). The originals are kept
in `textures.original/`. Each level's parts (the balls, the modules, the transformer) are remade the same way into
its own `game/` folder, with its sky, particles, shadow and sounds, so the level plays without the copy's level
files (the game's open-format.md). Original levels
are made again from the copy on each run. A texture that repeats as a tile grows with its other side around it, so no seams show where tiles meet.
With `--relief S` each lit texture also gets a normal map of strength S made from its own picture, which the game
uses for remastered levels. It is off by default: guessed from the picture, the relief deepens every crack and gap
(at 8 the world looked etched by acid); keep it low, 1 to 3. Masonry is
always made: where a texture shows bricks in concrete (the floors' brick bands), the bricks are told by their colour
(terracotta among beige or grey), raised, the mortar between them sunk, and the game draws it with parallax, so the
bricks stand out at a glancing view too; wood, stone and metal are left alone. `relief DIR` makes the relief of one
level folder of the open format again, without remastering it. `--model` picks another Real-ESRGAN model put into its `models/` folder (the default, realesrgan-x4plus, comes with
it). `remaster` (or `sounds` alone) also makes the "original" sound theme in the game's data folder from the copy's
sounds and music; it is the original's, so it stays on the player's machine. Run it again after a newer toolbox to bring the levels up to date.

Beside every large picture (512 x 512 and up) `remaster` also writes it ready for the graphics card (`name.dtx`:
BC1 for an opaque colour picture, BC7 for one with alpha and for a normal map, with all mip levels; our own encoder).
The game loads these instead of the PNGs: a level in 4K then loads in a couple of seconds and takes about a fifth of
the video memory. The PNGs stay (for the next run, and the game falls back to them); a picture grown once in a run is
copied for every part that carries the same one, so the run is shorter and the game uploads it once. Levels remastered
by an older toolbox have none: run `remaster` again.

`import-maps DIR` sorts a downloaded collection of community maps (an unpacked archive: folders of any depth,
pictures, documents) into the game's `levels/user-levels/` (or `--to DIR`), in the shape the game lists (the game's
levels.md, "A level is a folder"): one folder per map with its `.nmo`, its picture beside it as `preview.jpg`/`.png`,
and a `level.txt` saying where it came from (and its whole title when the folder's name had to lose characters). A
generic file name (`Level_07.NMO` in "45-Degree Rotated Maps") is named after its collection ("45-Degree Rotated -
Level 07"); the same file found twice becomes one level; packed archives (`.tar.gz`, `.zip`...) are not opened but set aside (below);
names are made safe for Linux, Windows and macOS (no reserved names, no trailing dots or spaces, at most 40
characters cut at a word with "…", a second "Hello Mars" becomes "Hello Mars (2)"). Links are never followed, depth
and counts are capped, the source is only read, and each level is written into a hidden folder and renamed into
place. A second run adds only what is new. The window has the same as a field and an Import button.
Each path field in the window has a Browse… button (the game copy also Image… for a CD image: .cue, .bin, .iso) that opens the system's chooser; the path is checked like a typed one and the folder is remembered. On Linux it needs xdg-desktop-portal with a backend (-gtk, -kde, -gnome); without one the field stays typed.
Unpack a download outside `levels/user-levels/` (in Downloads, say): a source inside the folder it imports into is
refused. A folder of the download that cannot be read is skipped and named; names printed have control characters
escaped.

After the maps are made, the toolbox asks the game (`drahtseilakt` beside it or on the PATH) to check each one: the result lists every map with a verdict (plays fully, plays with differences, may not be passable, does not load) and what the game does not support yet in it, such as script blocks it does not know, script-driven parts that stand still, textures the author did not include, sounds of the scripts. The check is kept beside each map as `report.txt` and shown in the game's list of downloaded maps. Without the game at hand the maps are still imported; run `drahtseilakt --audit-report DIR` later.

With `--catalogue FILE` (the community's catalogue, `CustomMapList.md` of
[BallanceCustomMaps](https://github.com/pseudoacetyl/BallanceCustomMaps)) each map is looked up by its file's name
and its folder's (case, punctuation, spaces, full-width characters and "v1.0"/"V1.0"/"1.0" do not matter; a version
number the catalogue lacks is tried without, never as another version; a name two maps share, or one of fewer than
three letters, matches nothing; a folder's name counts only for the one map in it). A map found gets in its
`level.txt` `title =` (the English name), `title.zh =` (the original name, when it is another one in Chinese),
`author =` and `difficulty =`: the game's scale is 1–10, so **the catalogue's 1–5 (with halves) is doubled** (3.5
becomes 7; a range such as 2.5–3 takes its midpoint, rounded up: 6); no difficulty is written when the catalogue has
none. With a catalogue, a map neither in it nor with a readme beside it (a `.txt`, `.md` or `.nfo`, or a document
named as a readme or as the map) is not imported: like packed archives, it is copied into the **unsorted folder**
(`--unsorted DIR`, by default `community-unsorted` beside the levels folder), under its path in the download, and a
`README.txt` there lists each item and why it was set aside, to be looked at by hand. A map with a readme but not in
the catalogue is imported with the title from its name and no difficulty. The catalogue is read as a stranger's
file: at most 4 MiB, 20 000 rows and 1024 characters a cell, malformed rows skipped, values cleaned.

Sets of modified originals (a folder whose name has 45-degree / rotat, reverse / 反向 or invisible / 隐形) are not community
maps: `import-maps` puts them into `levels/modified/<Set>/<file stem>/<file>.nmo`, beside `levels/user-levels`, the layout
the game reads. Pro Racing and other new tracks stay community maps.

`import-maps --refresh DIR --catalogue FILE --source DIR` brings a levels folder imported before (`levels/user-levels`)
up to the catalogue in place: each level's `level.txt` is written from the catalogue (looked up by its `source =`
line and the folder's name; its comment, its `source =` line and other lines are kept), and a level folder that is
not a single level (no map, or several), or whose map is neither in the catalogue nor has a readme (in its folder, or
beside its source in the collection `--source DIR`, which the `source =` paths are relative to), is moved into the
unsorted folder (renamed on the same disk; nothing is deleted). Only folders import-maps made (a level.txt with a
`source =` line) are ever moved; a level whose source cannot be looked at is kept. Lines the catalogue has no value
for (an `author =` of your own) are kept.

The rules: the toolbox works on the player's own data only; it never ships game data (the sound themes and remastered levels it makes stay on the player's machine), never goes online, never touches the player's original copy, and leaves the game's physics alone (what it adds to a level is only drawn).

## Downloads and release builds

The toolbox has its own releases at [github.com/eugene-vereshchagin/drahtseilakt-toolbox/releases](https://github.com/eugene-vereshchagin/drahtseilakt-toolbox/releases) (the version is 0.1.0; the first release is not published yet). It is also inside the game's portable archives and the game's Windows installer (a Start menu shortcut); the overall picture of what is shipped where is in the game's [docs/drahtseilakt/distribution.md](https://github.com/eugene-vereshchagin/drahtseilakt/blob/main/docs/drahtseilakt/distribution.md).

Releases are built by GitHub Actions (`.github/workflows/toolbox-release.yml`, started by hand or by a tag `X.Y.Z`; the
tag makes a draft release with every package and `SHA256SUMS`, which the owner publishes). The file names carry no version (it stays in the tag, the title and the
packages' metadata), so `https://github.com/eugene-vereshchagin/drahtseilakt-toolbox/releases/latest/download/<name>` always gives the newest release:

| System | Files |
|---|---|
| Windows x86_64, arm64 | `drahtseilakt-toolbox-windows-<arch>-setup.exe` (installer), `drahtseilakt-toolbox-windows-<arch>.zip` (portable) |
| Linux x86_64, aarch64 | `drahtseilakt-toolbox-linux-<arch>.tar.gz` (portable), `drahtseilakt-toolbox_<amd64\|arm64>.deb`, `drahtseilakt-toolbox.<x86_64\|aarch64>.rpm` |
| macOS arm64, x86_64 | `drahtseilakt-toolbox-macos-<arch>.tar.gz` (portable; not signed: see the `README.txt` inside) |
| FreeBSD x86_64 | `drahtseilakt-toolbox-freebsd-x86_64.tar.gz` (experimental) |

Every package carries the game's OFL fonts in `fonts/` beside the program (the window reads them from there), so no
package depends on the game's. Real-ESRGAN (pinned and checked against its SHA-256, `third-party.txt`) is bundled on
Linux x86_64, Windows x86_64 and macOS; on the other systems the package's `README.txt` says it is missing, and the
toolbox then enlarges textures with Lanczos. The `.deb` and `.rpm` keep the program, its fonts and Real-ESRGAN in
`/usr/lib/drahtseilakt-toolbox/` with a link in `/usr/bin`. `scripts/ci-package.sh OS ARCH BINARY VERSION` makes the
packages from a built binary.

## Building

The toolbox shares the level reading with the game's crates, used by path: it needs a checkout of the game's repository beside this one,
at `../drahtseilakt`. Then `cargo build --release --offline` (no network is needed; dependencies are pinned in `Cargo.lock`). For Real-ESRGAN, unpack the pinned download
into `third_party/realesrgan/` and link `target/release/realesrgan` to it. The game's repository is private: CI checks it out with the
secret `GAME_DEPLOY_KEY` (an SSH deploy key with read-only access to it).

## Who made it

The code and the documentation are written by **Claude** (Anthropic's AI model, working in Claude Code). **Veresk**
leads the project: the idea, the decisions, the plan and the judgement of what is right. The commits carry both
names.

GPL-3.0-only, as the game. What it uses of others (the fonts it reads, Real-ESRGAN, the crates) and their licences:
[THIRD-PARTY.md](THIRD-PARTY.md).
