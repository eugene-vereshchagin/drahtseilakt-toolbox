# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · **Deutsch** · [简体中文](README.zh.md)

Bereitet Level für [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), die neue Ballance-Engine,
auf dem Rechner des Spielers und aus seiner eigenen Kopie des Spiels vor. Das Spiel spielt nur, was in seinen
Level-Ordnern liegt; alles Schwere, was Level modern aussehen lässt, lebt hier, und das Spiel braucht nichts davon.

```
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--data DIR]
```

`remaster` wandelt die Original-Level der Kopie nach `levels/remastered/` des Spiels (offenes Format), gibt jedem
Level rundere Formen (ins Level neben die ursprünglichen Meshes geschrieben, die die Physik weiter benutzt, sodass
Läufe gleich bleiben) und vergrößert die Texturen N-fach (Standard 4; 1 lässt sie). Mit Real-ESRGAN gewinnen die
Texturen Details, sonst werden sie mit Lanczos skaliert (wie Texturen unter 64 Pixeln, denen es Details erfinden
würde). Real-ESRGAN wird in `realesrgan/` neben dem Programm der
Toolbox gesucht, dann im PATH; die benutzte Version ist in [third-party.txt](third-party.txt) festgelegt. Die
Originale bleiben in `textures.original/`. Die Teile, die alle Level teilen (Kugeln, Module, Transformator), werden
ebenso in den versteckten Ordner `levels/remastered/.shared/` umgearbeitet, und das Spiel zeichnet sie in
Remaster-Leveln so. Original-Level werden bei jedem Lauf neu aus der Kopie gebaut. Eine Textur, die sich als Kachel wiederholt, wächst mit ihrer Gegenseite ringsum, sodass an den Kachelgrenzen keine
Nähte entstehen. `--model` wählt ein anderes Real-ESRGAN-Modell aus seinem Ordner `models/` (Standard ist
realesrgan-x4plus, das mitkommt). Nach einer neueren Toolbox erneut ausführen, um die Level zu erneuern.

Die Toolbox geht nie online und fasst die Originalkopie des Spielers nie an.

## Bauen

Das Lesen der Level teilt die Toolbox mit dem Spiel: Sie braucht einen Klon des Spiel-Repositorys daneben, unter
`../ballance/drahtseilakt`. Dann `cargo build --release --offline`. Für Real-ESRGAN den festgelegten Download nach
`third_party/realesrgan/` entpacken und `target/release/realesrgan` darauf verlinken.

GPL-3.0-only, wie das Spiel.
