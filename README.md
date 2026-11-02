# drahtseilakt-toolbox

**English** · [Русский](README.ru.md) · [Deutsch](README.de.md) · [简体中文](README.zh.md)

Prepares levels for [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), the new Ballance engine,
on the player's own machine and from the player's own copy of the game. The game only plays what is in its level
folders; everything heavy that makes levels look modern lives here, so the game needs none of it.

```
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--data DIR]
```

`remaster` converts the original levels the copy has into the game's `levels/remastered/` (the open format), gives
each level rounder shapes (written into the level beside the original meshes, which the physics keeps using, so runs
are the same) and makes its textures N times larger (default 4; 1 leaves them). With Real-ESRGAN the textures gain
detail, else they are resampled with Lanczos (as are textures under 64 pixels, where it would invent detail). Real-ESRGAN is looked for in `realesrgan/` beside the toolbox's
program, then on the PATH; the version used is pinned in [third-party.txt](third-party.txt). The originals are kept
in `textures.original/`. The parts all levels share (the balls, the modules, the transformer) are remade the same
way into the hidden `levels/remastered/.shared/`, and the game draws them so in remastered levels. Original levels
are made again from the copy on each run. A texture that repeats as a tile grows with its other side around it, so no seams show where tiles meet.
Each lit texture also gets a normal map made from its own picture (rivets, plank gaps and ropes catch the light),
which the game uses for remastered levels. `--model` picks another Real-ESRGAN model put into its `models/` folder (the default, realesrgan-x4plus, comes with
it). Run it again after a newer toolbox to bring the levels up to date.

The toolbox never goes online and never touches the player's original copy.

## Building

The toolbox shares the level reading with the game: it needs a checkout of the game's repository beside this one,
at `../ballance/drahtseilakt`. Then `cargo build --release --offline`. For Real-ESRGAN, unpack the pinned download
into `third_party/realesrgan/` and link `target/release/realesrgan` to it.

GPL-3.0-only, as the game.
