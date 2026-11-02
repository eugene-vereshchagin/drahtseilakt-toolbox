# drahtseilakt-toolbox

**English** · [Русский](README.ru.md) · [Deutsch](README.de.md) · [简体中文](README.zh.md)

Prepares levels for [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), the new Ballance engine,
on the player's own machine and from the player's own copy of the game. The game only plays what is in its level
folders; everything heavy that makes levels look modern lives here, so the game needs none of it.

```
drahtseilakt-toolbox                     (the window)
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--relief S] [--data DIR]
drahtseilakt-toolbox relief DIR [--scale N] [--relief S]
```

Started without a command it opens its window (Linux X11 or Wayland, Windows, macOS; it remembers the model and
scale chosen last, in `toolbox.txt` in the game's data folder): your copy of the game, the
scale, the Real-ESRGAN model, the relief, a Make button, the progress per level and part with Cancel, the result and
a texture before and after. Its texts are in English, Russian, German and Simplified Chinese, from the system's
language or chosen in the window; Chinese uses a font the system has (Noto CJK, Microsoft YaHei, PingFang...).

`remaster` converts the original levels the copy has into the game's `levels/remastered/` (the open format), gives
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

The toolbox never goes online and never touches the player's original copy.

## Building

The toolbox shares the level reading with the game: it needs a checkout of the game's repository beside this one,
at `../ballance/drahtseilakt`. Then `cargo build --release --offline`. For Real-ESRGAN, unpack the pinned download
into `third_party/realesrgan/` and link `target/release/realesrgan` to it.

GPL-3.0-only, as the game.
