// drahtseilakt-toolbox: prepares levels for the game Drahtseilakt on the player's own machine, from their own copy.
// The game only plays what is in the level folders; making remastered levels (rounder shapes, larger textures) and
// whatever heavy work later comes with it lives here, so the game needs none of it (the game's
// docs/drahtseilakt/decisions.md, "The toolbox prepares levels"). It never goes online either; Real-ESRGAN, which its
// download carries (third-party.txt), or the player's own, is used when it is there.

// Hostile input is parsed only by the `nmo` and `level` crates; this crate works on validated models, and geometry
// and image code reads better with plain arithmetic and indexing of fixed-size arrays.
#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

mod bc;
mod catalogue;
mod gui;
mod import;
mod masonry;
mod normals;
mod remaster;
mod smooth;
mod texts;
mod theme;
mod unsorted;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: drahtseilakt-toolbox [<command>] [options]

Without a command the toolbox opens its window.

commands:
  remaster            convert the original levels the copy has into levels/ballance-remaster (the open format), give
                      every level there its rounder shapes and make its textures larger; the game lists them in
                      Custom levels. Run it again after a newer toolbox to bring the levels up to date.
                      Brickwork in concrete is told by its colour and drawn with depth (masonry relief).
  sounds              make only the 'original' sound theme from the copy's sounds and music (remaster makes
                      it too); it stays in the game's data folder, never shared
  remaster-open DIR   the remaster of one open-format level folder in place (no copy of the original needed): rounder
                      shapes, larger textures (--scale, --model), masonry relief (--relief); the plain originals stay
                      in textures.original/, so running it again does not grow anything twice. Looks only: physics
                      and replays stay the same.
  remaster-nmo FILE   the remaster of one community map (.nmo): the map exported as an open-format level into
                      FILE.remaster beside it (looks and pictures only; the game plays the map itself and draws
                      its meshes in these looks), then rounded and enlarged as remaster-open does. The map file
                      is only read. Back to 2004 draws the map as it is.
  relief DIR          make one remastered level folder's relief again (masonry, and --relief if given)
  import-maps DIR     sort a downloaded collection of community maps (an unpacked archive, folders of any depth)
                      into the game's levels/user-levels: one folder per map with its .nmo, its preview picture
                      and a level.txt with its title; the same map twice becomes one level; packed archives are
                      copied into the unsorted folder, not opened; DIR is only read. --to DIR imports elsewhere.
                      With --catalogue FILE each map found in it gets its English title, Chinese title, author
                      and difficulty, and a map neither in it nor with a readme beside it is copied into the
                      unsorted folder instead. Afterwards the game checks every map it made and says plainly what
                      it does not support yet in it (plays fully / with differences / may not be passable); the
                      check is kept beside each map as report.txt and shown in the game's list of downloaded maps
  import-maps --refresh DIR --catalogue FILE [--source DIR]
                      bring a levels folder imported before (levels/user-levels) up to the catalogue in place:
                      level.txt files rewritten from it; level folders that are not one level, or whose map is
                      neither in the catalogue nor has a readme (in the folder, or beside its source in the
                      collection --source DIR, which level.txt's source = line names a path in), moved into the
                      unsorted folder. Nothing is deleted

options:
  --scale N           make the textures N times larger (2 to 32, default 4; 1 leaves them as they are). With
                      Real-ESRGAN (in realesrgan/ beside the toolbox, as its download carries it, or
                      realesrgan-ncnn-vulkan on the PATH) they gain detail, else they are resampled with Lanczos
                      and sharpened a little. The originals are kept in textures.original/.
  --model NAME        the Real-ESRGAN model (default realesrgan-x4plus, which comes with it): any model whose
                      NAME.param and NAME.bin are in its models/ folder
  --relief S          give lit textures normal maps of strength S (0 to 20, default 0: none); the relief is
                      guessed from the picture, so every crack and gap is deepened: keep it low (1 to 3)
  --levels LIST       with remaster: only these levels (e.g. 1,7 or Level_01; default: every one)
  --to DIR            with import-maps: the folder to make the levels in (default the game's levels/user-levels)
  --catalogue FILE    with import-maps: the community's catalogue (CustomMapList.md of BallanceCustomMaps)
  --unsorted DIR      with import-maps: where what does not fit is set aside, with a README.txt saying why
                      (default community-unsorted beside the levels folder)
  --source DIR        with import-maps --refresh: the collection the levels were imported from
  --refresh DIR       see import-maps
  --data DIR          the Ballance folder (default: $OPENBALLANCE_DATA, then the copy the game imported)
  -h, --help          this text
";

struct Options {
    command: Option<String>,
    scale: u32,
    model: String,
    relief: f32,
    /// `--levels`: the remastered level folders to make (empty: all).
    levels: Vec<String>,
    data: Option<PathBuf>,
    /// The folder a command works on (relief, import-maps).
    folder: Option<PathBuf>,
    /// Where import-maps makes its levels.
    to: Option<PathBuf>,
    catalogue: Option<PathBuf>,
    unsorted: Option<PathBuf>,
    source: Option<PathBuf>,
    refresh: Option<PathBuf>,
}

fn options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut o = Options {
        command: None,
        scale: 4,
        model: "realesrgan-x4plus".into(),
        relief: 0.0,
        levels: Vec::new(),
        data: None,
        folder: None,
        to: None,
        catalogue: None,
        unsorted: None,
        source: None,
        refresh: None,
    };
    let mut args = args;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--scale" => {
                let n: u32 = value()?
                    .parse()
                    .map_err(|_| "--scale needs a number (1 to 32)".to_owned())?;
                o.scale = n.clamp(1, 32);
            }
            "--data" => o.data = Some(value()?.into()),
            "--levels" => {
                o.levels = value()?
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .take(64)
                    .map(str::to_owned)
                    .collect()
            }
            "--to" => o.to = Some(value()?.into()),
            "--catalogue" => o.catalogue = Some(value()?.into()),
            "--unsorted" => o.unsorted = Some(value()?.into()),
            "--source" => o.source = Some(value()?.into()),
            "--refresh" => o.refresh = Some(value()?.into()),
            "--model" => o.model = value()?,
            "--relief" => {
                let r: f32 = value()?.parse().map_err(|_| "--relief needs a number (0 to 20)".to_owned())?;
                o.relief = if r.is_finite() { r.clamp(0.0, 20.0) } else { 0.0 };
            }
            "-h" | "--help" => o.command = Some("help".into()),
            c if !c.starts_with('-') && o.command.is_none() => o.command = Some(c.to_owned()),
            c if !c.starts_with('-') && o.folder.is_none() => o.folder = Some(c.into()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(o)
}

fn main() -> ExitCode {
    let run = || -> Result<(), String> {
        let o = options(std::env::args().skip(1))?;
        match o.command.as_deref() {
            Some("remaster") => {
                let data = gamedata::assets::Data::locate(o.data)?;
                for line in remaster::run(&data, o.scale, &o.model, o.relief, &o.levels)? {
                    println!("{line}");
                }
                Ok(())
            }
            Some("sounds") => {
                // only the "original" sound theme, from the copy's sounds and music
                let data = gamedata::assets::Data::locate(o.data)?;
                println!("{}", remaster::original_sounds(&data, &gamedata::profile::data_dir().join("sounds"))?);
                Ok(())
            }
            Some("remaster-open") => {
                let dir = o.folder.ok_or("remaster-open needs the level's folder")?;
                for line in remaster::remaster_folder(&dir, o.scale, &o.model, o.relief)? {
                    println!("{line}");
                }
                Ok(())
            }
            Some("remaster-nmo") => {
                let file = o.folder.ok_or("remaster-nmo needs the .nmo file")?;
                for line in remaster::remaster_nmo(&file, o.scale, &o.model, o.relief)? {
                    println!("{line}");
                }
                Ok(())
            }
            Some("relief") => {
                // one remastered level's folder: its relief (masonry, and the brightness relief if asked) made again
                let dir = o.folder.ok_or("relief needs the level's folder")?;
                let n = remaster::relief_folder(&dir, o.scale, o.relief)?;
                println!("{}: {n} relief maps", dir.display());
                Ok(())
            }
            Some("import-maps") => {
                let catalogue = o.catalogue.as_deref().map(catalogue::read).transpose()?;
                if let Some(c) = &catalogue {
                    println!("catalogue: {} maps ({} rows skipped as malformed)", c.entries.len(), c.skipped);
                }
                let never = std::sync::atomic::AtomicBool::new(false);
                if let Some(dir) = o.refresh {
                    let c = catalogue.as_ref().ok_or("import-maps --refresh needs --catalogue FILE")?;
                    let unsorted = o.unsorted.unwrap_or_else(|| import::default_unsorted(&dir));
                    let r = import::refresh(&dir, c, o.source.as_deref(), &unsorted, &mut |l| println!("{l}"), &never)?;
                    for l in r.left.iter().chain(&r.failed) {
                        println!("{}", import::printable(l));
                    }
                    println!("{}", import::refresh_line(&r, &unsorted));
                    return Ok(());
                }
                let from = o.folder.ok_or("import-maps needs the folder of maps")?;
                let to = o.to.unwrap_or_else(|| gamedata::profile::dir(gamedata::profile::Kind::Downloaded));
                let sorting = import::Sorting {
                    catalogue: catalogue.as_ref(),
                    unsorted: o.unsorted.unwrap_or_else(|| import::default_unsorted(&to)),
                };
                let s = import::import(&from, &to, &sorting, &mut |l| println!("{l}"), &never)?;
                println!("{}", import::summary_line(&s, &to));
                if s.imported > 0 {
                    import::check_maps(&to, &mut |l| println!("{l}"));
                }
                Ok(())
            }
            Some("help") => {
                print!("{USAGE}");
                Ok(())
            }
            None => gui::run(o.data),
            Some(c) => Err(format!("unknown command {c}\n\n{USAGE}")),
        }
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("drahtseilakt-toolbox: {e}");
            ExitCode::FAILURE
        }
    }
}
