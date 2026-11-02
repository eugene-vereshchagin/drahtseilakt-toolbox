# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · **Deutsch** · [简体中文](README.zh.md)

Bereitet Level für [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt), die neue Ballance-Engine,
auf dem Rechner des Spielers und aus seiner eigenen Kopie des Spiels vor. Das Spiel spielt nur, was in seinen
Level-Ordnern liegt; alles Schwere, was Level modern aussehen lässt, lebt hier, und das Spiel braucht nichts davon.

```
drahtseilakt-toolbox                     (das Fenster)
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--relief S] [--data DIR]
drahtseilakt-toolbox relief DIR [--scale N] [--relief S]
drahtseilakt-toolbox import-maps DIR [--to DIR]
```

Ohne Befehl gestartet öffnet die Toolbox ihr Fenster (Linux X11 oder Wayland, Windows, macOS; Modell und Faktor
der letzten Wahl merkt es sich in `toolbox.txt` im Datenordner des Spiels): deine Kopie des
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
Originale bleiben in `textures.original/`. Die Teile jedes Levels (Kugeln, Module, Transformator) werden ebenso in seinen eigenen
Ordner `game/` umgearbeitet, mit Himmel, Partikeln, Schatten und Klängen, sodass das Level ohne die Leveldateien der
Kopie spielt (open-format.md des Spiels). Original-Level werden bei jedem Lauf neu aus der Kopie gebaut. Eine Textur, die sich als Kachel wiederholt, wächst mit ihrer Gegenseite ringsum, sodass an den Kachelgrenzen keine
Nähte entstehen. Mit `--relief S` bekommt jede beleuchtete Textur außerdem eine Normal Map der Stärke S
aus ihrem eigenen Bild, die das Spiel in Remaster-Leveln benutzt. Standardmäßig aus: aus dem Bild geraten, vertieft das
Relief jeden Riss und jede Fuge (bei 8 sah die Welt wie von Säure zerfressen aus); niedrig halten, 1 bis 3. Mauerwerk entsteht immer: wo eine Textur Ziegel in Beton zeigt (die
Ziegelbänder der Böden), werden die Ziegel an ihrer Farbe erkannt (Terrakotta zwischen Beige oder Grau), angehoben,
die Fugen dazwischen gesenkt, und das Spiel zeichnet es mit Parallax, sodass die Ziegel auch bei flachem Blick
hervortreten; Holz, Stein und Metall bleiben unberührt. `relief DIR` macht das Relief eines Level-Ordners im offenen
Format neu, ohne ihn zu remastern. `--model` wählt ein anderes Real-ESRGAN-Modell aus seinem Ordner `models/` (Standard ist
realesrgan-x4plus, das mitkommt). `remaster` (oder `sounds` allein) macht auch das Klangthema „original“ im Datenordner des Spiels aus den Klängen
und der Musik der Kopie; es ist das Original und bleibt auf dem Rechner des Spielers. Nach einer neueren Toolbox erneut ausführen, um die Level zu erneuern.

`import-maps DIR` sortiert eine geladene Sammlung von Community-Karten (ein entpacktes Archiv: Ordner beliebiger
Tiefe, Bilder, Dokumente) in `levels/community/` des Spiels (oder `--to DIR`) so, wie das Spiel sie zeigt (levels.md
des Spiels, „Ein Level ist ein Ordner“): ein Ordner je Karte mit ihrer `.nmo`, ihrem Bild daneben als
`preview.jpg`/`.png` und einer `level.txt`, die sagt, woher sie kommt (und den ganzen Titel, wenn der Ordnername
Zeichen verlieren musste). Ein nichtssagender Dateiname (`Level_07.NMO` in „45-Degree Rotated Maps“) wird nach seiner
Sammlung benannt („45-Degree Rotated - Level 07“); dieselbe Datei zweimal gefunden ist ein Level; gepackte Archive
(`.tar.gz`, `.zip`...) werden genannt, nicht geöffnet; Namen taugen für Linux, Windows und macOS (keine reservierten
Namen, keine Punkte oder Leerzeichen am Ende, höchstens 40 Zeichen, an einer Wortgrenze mit „…“ gekürzt, ein zweites
„Hello Mars“ wird „Hello Mars (2)“). Links werden nie verfolgt, Tiefe und Anzahl sind begrenzt, die Quelle wird nur
gelesen, und jedes Level wird in einen versteckten Ordner geschrieben und an seinen Platz umbenannt. Ein zweiter Lauf
fügt nur Neues hinzu. Das Fenster hat dasselbe als Feld und Import-Knopf.
Einen Download außerhalb von `levels/community/` entpacken (etwa in Downloads): eine Quelle im Zielordner wird
abgelehnt. Ein unlesbarer Ordner des Downloads wird übersprungen und genannt; ausgegebene Namen haben ihre
Steuerzeichen maskiert.

Die Toolbox geht nie online und fasst die Originalkopie des Spielers nie an.

## Bauen

Das Lesen der Level teilt die Toolbox mit dem Spiel: Sie braucht einen Klon des Spiel-Repositorys daneben, unter
`../ballance/drahtseilakt`. Dann `cargo build --release --offline`. Für Real-ESRGAN den festgelegten Download nach
`third_party/realesrgan/` entpacken und `target/release/realesrgan` darauf verlinken.

## Wer es gemacht hat

Code und Dokumentation schreibt **Claude** (das KI-Modell von Anthropic, in Claude Code). **Veresk** führt das
Projekt: die Idee, die Entscheidungen, der Plan und das Urteil, was stimmt. Die Commits tragen beide Namen.

GPL-3.0-only, wie das Spiel.
