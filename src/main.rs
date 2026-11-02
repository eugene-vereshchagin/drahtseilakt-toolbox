// drahtseilakt-toolbox: prepares levels for the game Drahtseilakt on the player's own machine, from their own copy.
// The game only plays what is in the level folders; making remastered levels (rounder shapes, larger textures) and
// whatever heavy work later comes with it lives here, so the game needs none of it (the game's
// docs/drahtseilakt/decisions.md, "The toolbox prepares levels"). It never goes online either; Real-ESRGAN, which its
// download carries (third-party.txt), or the player's own, is used when it is there.

// Hostile input is parsed only by the `nmo` and `level` crates; this crate works on validated models, and geometry
// and image code reads better with plain arithmetic and indexing of fixed-size arrays.
#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

mod gui;
mod masonry;
mod normals;
mod remaster;
mod smooth;
mod texts;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: drahtseilakt-toolbox [<command>] [options]

Without a command the toolbox opens its window.

commands:
  remaster            convert the original levels the copy has into levels/remastered (the open format), give
                      every level there its rounder shapes and make its textures larger; the game lists them in
                      Custom levels. Run it again after a newer toolbox to bring the levels up to date.
                      Brickwork in concrete is told by its colour and drawn with depth (masonry relief).
  relief DIR          make one remastered level folder's relief again (masonry, and --relief if given)

options:
  --scale N           make the textures N times larger (2 to 32, default 4; 1 leaves them as they are). With
                      Real-ESRGAN (in realesrgan/ beside the toolbox, as its download carries it, or
                      realesrgan-ncnn-vulkan on the PATH) they gain detail, else they are resampled with Lanczos
                      and sharpened a little. The originals are kept in textures.original/.
  --model NAME        the Real-ESRGAN model (default realesrgan-x4plus, which comes with it): any model whose
                      NAME.param and NAME.bin are in its models/ folder
  --relief S          give lit textures normal maps of strength S (0 to 20, default 0: none); the relief is
                      guessed from the picture, so every crack and gap is deepened: keep it low (1 to 3)
  --data DIR          the Ballance folder (default: $OPENBALLANCE_DATA, then the copy the game imported)
  -h, --help          this text
";

struct Options {
    command: Option<String>,
    scale: u32,
    model: String,
    relief: f32,
    data: Option<PathBuf>,
    /// The folder a command works on (relief).
    folder: Option<PathBuf>,
}

fn options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut o = Options {
        command: None,
        scale: 4,
        model: "realesrgan-x4plus".into(),
        relief: 0.0,
        data: None,
        folder: None,
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
                for line in remaster::run(&data, o.scale, &o.model, o.relief)? {
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
