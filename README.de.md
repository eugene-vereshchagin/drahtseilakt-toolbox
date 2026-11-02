# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · **Deutsch** · [简体中文](README.zh.md)

Bereitet Level für [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), die neue Ballance-Engine,
auf dem Rechner des Spielers und aus seiner eigenen Kopie des Spiels vor. Das Spiel spielt nur, was in seinen
Level-Ordnern liegt; alles Schwere, was Level modern aussehen lässt, lebt hier, und das Spiel braucht nichts davon.

```
drahtseilakt-toolbox                     (das Fenster)
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--relief S] [--data DIR]
```

Ohne Befehl gestartet öffnet die Toolbox ihr Fenster (Linux X11 oder Wayland, Windows, macOS): deine Kopie des
Spiels, die Vergrößerung, das Real-ESRGAN-Modell, das Relief, ein Knopf Erstellen, der Fortschritt je Level und Teil
mit Abbrechen, das Ergebnis und eine Textur vorher und nachher. Die Texte gibt es auf Englisch, Russisch, Deutsch
und vereinfachtem Chinesisch, nach der Systemsprache oder im Fenster gewählt; Chinesisch nutzt eine Schrift des
Systems (Noto CJK, Microsoft YaHei, PingFang...).

`remaster` wandelt die Original-Level der Kopie nach `levels/remastered/` des Spiels (offenes Format), gibt jedem
Level rundere Formen (ins Level neben die ursprünglichen Meshes geschrieben, die die Physik weiter benutzt, sodass
Läufe gleich bleiben) und vergrößert die Texturen N-fach (Standard 4; 1 lässt sie). Mit Real-ESRGAN gewinnen die
Texturen Details, sonst werden sie mit Lanczos skaliert (wie Texturen unter 64 Pixeln, denen es Details erfinden
würde). Real-ESRGAN wird in `realesrgan/` neben dem Programm der
Toolbox gesucht, dann im PATH; die benutzte Version ist in [third-party.txt](third-party.txt) festgelegt. Die
Originale bleiben in `textures.original/`. Die Teile, die alle Level teilen (Kugeln, Module, Transformator), werden
ebenso in den versteckten Ordner `levels/remastered/.shared/` umgearbeitet, und das Spiel zeichnet sie in
Remaster-Leveln so. Original-Level werden bei jedem Lauf neu aus der Kopie gebaut. Eine Textur, die sich als Kachel wiederholt, wächst mit ihrer Gegenseite ringsum, sodass an den Kachelgrenzen keine
Nähte entstehen. Mit `--relief S` bekommt jede beleuchtete Textur außerdem eine Normal Map der Stärke S
aus ihrem eigenen Bild, die das Spiel in Remaster-Leveln benutzt. Standardmäßig aus: aus dem Bild geraten, vertieft das
Relief jeden Riss und jede Fuge (bei 8 sah die Welt wie von Säure zerfressen aus); niedrig halten, 1 bis 3. `--model` wählt ein anderes Real-ESRGAN-Modell aus seinem Ordner `models/` (Standard ist
realesrgan-x4plus, das mitkommt). Nach einer neueren Toolbox erneut ausführen, um die Level zu erneuern.

Die Toolbox geht nie online und fasst die Originalkopie des Spielers nie an.

## Bauen

Das Lesen der Level teilt die Toolbox mit dem Spiel: Sie braucht einen Klon des Spiel-Repositorys daneben, unter
`../ballance/drahtseilakt`. Dann `cargo build --release --offline`. Für Real-ESRGAN den festgelegten Download nach
`third_party/realesrgan/` entpacken und `target/release/realesrgan` darauf verlinken.

GPL-3.0-only, wie das Spiel.
