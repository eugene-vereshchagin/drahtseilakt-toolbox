# drahtseilakt-toolbox — working rules

The toolbox prepares levels for the game Drahtseilakt (the repository beside this one, `../drahtseilakt`,
whose CLAUDE.md, roadmap and decisions apply here too; the decision "The toolbox prepares levels; the game only
plays" is why this program exists). Owner: Veresk. Chat in Russian; code, comments, docs and commits in English.

- **The player's own data only.** The toolbox reads the player's copy (through the game's `gamedata` crate) and
  writes only into the game's `levels/ballance-remaster/`, its sound themes, and (`import-maps`, from a download the player
  names, which it only reads) `levels/user-levels/`. It never touches the original copy, never ships game data, and
  never goes online. Third-party programs it runs (Real-ESRGAN) are pinned in `third-party.txt` (URL, version,
  SHA-256), unpacked into `third_party/` (never committed) and looked for beside the program, then on the PATH.
- **No font compiled in** (eframe without `default_fonts`): the window reads the game's OFL fonts at run time
  (`src/theme.rs`). `THIRD-PARTY.md` lists fonts, programs and crates; regenerate its crate part after a Cargo.lock
  change (the command is in the file).
- **Security** (secure-codegen rules, as the game): no `unsafe`; no `unwrap`/`expect` on input-derived values;
  level folders are hostile (shared by strangers): never follow or write through links, write beside and move into
  place, fail closed. Parser and file-layer changes are level C: independent review before commit.
- **Physics untouched.** What the toolbox adds to a level (a mesh's `look`) is only drawn; the physics and rules of
  the game keep the original meshes, so runs and replays stay the same.
- **Docs** in English, Russian, German and Simplified Chinese (`README.<lang>.md`), changed with the behaviour.
- **Builds** offline: `cargo build --release --offline`, dependencies pinned in Cargo.lock; the game's crates by path.
- Commits authored as `Veresk <eugene-vereshchagin@users.noreply.github.com>`. Never release, tag or publish without
  the owner's explicit command.
