# Koop-Modus (Devil Within Mod)

Lokaler Zwei-Spieler-Koop für Devil Within: Spieler 2 steigt mit dem zweiten Controller als
zweiter Jin ins laufende Spiel ein. Vorgehen nach der Anleitung
[coop-anything](https://github.com/cranebeaker/coop-anything) (Weg 2 „von null bauen“, Abschnitt
„Decomp or recomp“: bei einer statischen Rekompilation kommt Koop über die Mod-Hooks ins Spiel).
Code: `rust/crates/t5-recomp/src/devil_mod.rs`, Abschnitt „Co-op“ (`coop_update`, `pad_as_game`).

## Spielen

- Zwei Gamepads (`play.bat` / `play.sh`): Port 1 = Tastatur/Maus + das zuerst benutzte
  Gamepad, Port 2 = das als zweites benutzte Gamepad (`t5-bevy/src/input.rs`).
- Tastatur + ein Gamepad: **F4** oder Einstellungsmenü (**P**) „Gamepad port“ schaltet das
  Gamepad auf Port 2 (Spieler 2) und zurück (Meldung „Gamepad: port 2“ oben rechts); Port 1 ist
  dann nur Tastatur/Maus. `T5_PAD_P2=1` startet gleich so.
- **Friendly Fire:** Einstellungsmenü (**P**) „Friendly fire“. An: Spieler 2 bekommt eine eigene
  Team-ID (die erste, die keine andere Figur hat), sodass sich die Spieler gegenseitig treffen und
  die Gegner beide angreifen; aus: Team von Spieler 1. Das Spiel lässt nur Figuren
  verschiedener Teams einander treffen (0x16F930 vergleicht die Team-IDs, Figurdaten +0x300A).
  Beide Menüpunkte gelten nur für die laufende Sitzung.
- **Beitreten:** Spieler 2 drückt **START** im Spiel (kein Menü offen). Ein zweiter Jin erscheint
  neben Spieler 1 (Meldung „Player 2 joined“ oben rechts). Der Konami-Code/Debug-Modus ist dafür
  **nicht** nötig.
- **SELECT:** Spieler 2 springt zu Spieler 1 (z. B. wenn er aus dem Bild gelaufen ist).
- **SELECT + START:** Spieler 2 verlässt das Spiel (seine Figur wird entfernt).
- Stirbt Spieler 2, gibt es kein Game Over; „Player 2 is down“, mit START steigt er neu ein.
- Spieler 2 benutzt dieselbe Tastenbelegung und dieselben Stick-Optionen wie Spieler 1
  (Spieloptionen von Port 1).

## Wie es funktioniert

Devil Within liest im Kampf nur **einen** Controller in eine Figur:

- Eingabe-Datensatz (0x15C6A0, Index Figur +0x140), +0 Controller: `0` = Pad 1 (0x15C4E0),
  `< 0` = KI-Puffer, sonst keine Eingabe. Ein Wert für „Pad 2“ gibt es nicht.
- Das Update jeder KI-Figur (0x1493F8) setzt den globalen Zeiger [0x41BED0] auf den Pad-Puffer
  ihrer KI (0x15E5F8(0x174720(ai)), Puffer = KI-Datensatz +0xB60), lässt die KI denken
  (0x1746D0 → 0x174BE8, schreibt den Puffer über 0x17EB80) und liest dann den Datensatz
  (0x15CF40 → 0x15C6A0): KI-Figuren benutzen denselben Eingabeweg wie der Spieler.
- KI-Datensatz (Pool \*0x4804CC, Schritt [0x4804C0], Anzahl [0x4804BA], Index Figur +0x148):
  **+0 Bit 4 = KI angehalten** – 0x174BE8 kehrt dann sofort zurück und schreibt den Puffer nicht.

Die Mod nutzt das ohne Eingriff in den generierten Code (jeder Vsync, `coop_update`):

1. Beitreten: derselbe Ablauf wie „Spawn“ im Mod-Menü (MAN_INFO 0x148A70 + MAN_CREATE 0x148968,
   Profil Jin 0x31E, 800 Einheiten neben Spieler 1). Die Figur hat das KI-Bit (+0x154 Bit 2):
   ihr Tod ist kein Game Over.
2. Jeden Vsync: Team = Team von Spieler 1 (+0x300A der Figurdaten), Controller des Datensatzes
   < 0, KI angehalten (Bit 4), und in den KI-Puffer kommt Port 2 **im selben Format, das 0x15C6A0
   aus Pad 1 baut** (`pad_as_game`):
   - Tasten: Pad-Zustand des Spiels je Port (0x101FC0, 0x2A98A0 + 100 × Port: +0x40 Tasten,
     +0x5C Controllertyp, +0x60 Status) → Tastenbelegung 0x1021F0 (0x425833 / 0x42583B,
     Steuerkreuz nur ohne Analog-/DualShock-2-Typ 0x73/0x79) → Bitordnung 0x15C4E0.
   - Sticks: +0 / +4 / +8 / +0xC (wie 0x2ACE60/64/68/6C für Pad 1) → Totzone 0x15C680 und
     Achsen-Umkehr nach der Stick-Option von Port 1 (0x104FF0: [[0x41ADF4]+0x38] + Port × 4 +
     0xC, Bits 7–8). Puffer +4/+8/+0xC/+0x10 → Datensatz +0x10/+0x14/+0x18/+0x1C.
   - Welcher Port Spieler 2 ist: der, von dem Pad 1 nicht gelesen wird (Byte 0x425822).
3. In Menüs (Pause, Ergebnisse) und bei offenem Mod-Menü bekommt Spieler 2 keine Eingabe.

Port 2 liefert die Laufzeit schon (padman-HLE bedient beide Ports, `ps2-runtime/src/iop/pad.rs`;
das Spiel liest beide Ports jeden Frame über 0x1024F8 → 0x101FC0).

## Splitscreen (Prototyp, `T5_DW_SPLIT=1`)

- Die Kamera ist ein Einzelstück ([0x41BD10]); ihr Datenblock (Pool 0x42B6B4, Schritt
  [0x42B6A8]) wird nach **jedem fertigen Spielbild** gegen den zweiten getauscht (`split_swap`;
  der zweite beginnt als Kopie mit Ziel +0xF0/+0xF4 = Figur von Spieler 2). Die Bilder wechseln
  so zwischen den Kameras, jede wird jedes zweite Bild weitergerechnet.
- „Fertiges Spielbild“ = `raster::RENDERED_TOTAL` (Kopie des fertigen Bildes auf den angezeigten
  Puffer). Devil Within wechselt den Anzeigepuffer nie, der Flip-Zähler bleibt 1 – daran
  scheiterte der erste Versuch (Nutzer: „funktioniert gar nicht“).
- Die Laufzeit markiert jedes ausgegebene Bild mit der Kamera, die es zeichnete
  (`ps2_runtime::SPLIT_VIEW` → `gs::Frame::view`, `FrameShared::by_view` hält das letzte je
  Ansicht); das Fenster setzt links Spieler 1, rechts Spieler 2 zusammen (je die Bildmitte).
  `T5_DW_SPLIT_LAG=n` verschiebt die Zuordnung um n Bilder (0 passt, Nutzertest).

Nach dem ersten Nutzertest (Kameras träge, Laufen komisch, Räume/Gegner nur bei Spieler 1, Sound
komisch) gebaut, **noch nicht getestet**:

- **Ansicht = Datenblock + Kamera-Task-Felder** (`View`): Matrix/Position +0x60..+0x9F
  (0x153BB0) und Zelle +0xEC/+0xF0. Das Kamera-Update ruft 0x123480, das die Zelle der Kamera aus
  ihrer Position sucht, **ausgehend von der bisherigen** (0x129138); mit einer Zelle für beide
  Ansichten blieb sie bei Spieler 1 → Räume bei Spieler 2 wurden nicht gezeichnet.
- **Träge Kameras:** Am Bildanfang (Thread-Haken 0x20F110, neu `kernel::request_thread_native`
  für Rust-Code im Spiel-Thread) wird die versteckte Ansicht eingesetzt, mit 0x154190
  weitergerechnet und wieder ausgetauscht – jede Kamera bewegt sich jedes Bild.
- **Laufen:** Das Spiel addiert den Kamerawinkel (Datenblock +0xA4, 0x10000 = Vollkreis) zum
  Stickwinkel; der läuft **im Uhrzeigersinn** (0x15C6A0 mit 0x21F2F8: oben 0x8000, rechts
  −0x4000). Die Laufzeit dreht den linken Stick je Port (`pad::TURN`, in der IOP-Pad-Ausgabe)
  um aktiven Winkel − eigenen Winkel gegen den Uhrzeigersinn. Der erste Versuch drehte falsch
  herum (richtig nur jedes zweite Bild).
- **„Der Spieler“** des Spiels (ManMgr-Eintrag +0, 0x147A98) wechselt mit der Kamera: Spieler 2 in
  seinen Bildern → **eigenes HUD für Spieler 2 (Nutzertest: funktioniert)**, Raumskripte
  (Seriennummer 0x147AF0) sehen beide. `player()` der Mod liefert weiter Spieler 1
  (`SPLIT_PLAYER1`).
- **Gegner-Ziel** = Figurdaten **+0x3030** (Seriennummer, −1 keins; +0x302C Modus), gesetzt von
  den Raumskripten (0x16C168/0x16C188 über 0x199578/0x199598) mit „dem Spieler“ zur Zeit des
  Skripts. Die Mod gibt jedem Gegner, dessen Ziel ein Spieler ist, den näheren (Wechsel erst ab
  300 Einheiten Vorsprung, `retarget_enemies`; auch ohne Splitscreen).
- **Sound:** Das Kamera-Update setzt auch den 3D-Hörer (0x153BB0 → 0x101670 → 0x1335D8 →
  nuSound3D 0x253590; Hörer 0x69B3C0..+0x30, Kopie 0x429A20..+0x3C, Eintrag [0x41EF0C]). Beim
  zusätzlichen Update von Spieler 2s Kamera wird er gesichert und zurückgesetzt, so steht er am
  Bildanfang immer auf Spieler 1. In Spieler 2s Bildern setzt ihn das Spiel-Update noch auf
  Spieler 2 (Reihenfolge Kamera/Sound im Bild unbekannt) – Test nötig.
- **16:9:** Fenster 1600×900 bei `T5_DW_SPLIT`; jede Hälfte = mittlere **2/3** des Spielbilds
  (8:9 bei 4:3, nichts gestreckt). Beide Kameras im Splitscreen um **1,35** zurückgezogen
  (Abstand/Höhen +0x10/+0x0C/+0x20 je Ansicht, Spielwerte gemerkt, `split_zoom`); die
  Boss-Kamera-Skalierung pausiert solange.
- **Befund für später:** Die Engine kann mehrere Ansichten nativ (Anzahl [0x2ACD38], 0x101590,
  bis 4; Ansichtsblöcke 0x2ACDD0 + i×0x1B0; Renderschleife 0x100E70 zeichnet die Szene je
  Ansicht, HUD danach einmal). Damit wären beide Kameras im selben Bild mit 60 fps möglich statt
  abwechselnd – offen: Zellen/HUD je Ansicht, Leistung.

## Testen

Skript-Eingaben (`t5-headless` und Fenster `t5-bevy`): `--press P2:BUTTON@SEKUNDE[+HALTEN]`
drückt auf Pad 2 (Port 2 gilt dann als verbunden), `LSTICK_RIGHT` / `_LEFT` / `_UP` / `_DOWN`
kippen den linken Stick voll (auch mit `P2:`). `T5_DW_COOPLOG=1` schreibt jeden Vsync mit
Eingabe von Spieler 2 ins Log: gesendeter Puffer, Datensatz von Spieler 2 und Spieler 1,
Positionen.

Ins Spiel (Speicherkarte mit Spielstand, `run/dwmod.sh`, Warp 0–40):

```bash
P="--press CIRCLE@75 --press CIRCLE@82 --press CIRCLE@110 --press START@120 --press CIRCLE@141
   --press CIRCLE@145 --press CIRCLE@150 --press START@158 --press CIRCLE@165"
T5_DW_COOPLOG=1 bash run/dwmod.sh coop 203 $P --press P2:START@185 --press P2:LSTICK_RIGHT@191+3 \
    --press LSTICK_RIGHT@191+3 --press P2:LSTICK_UP@195+3 --press P2:SELECT@200
```

## Stand (26.10., headless geprüft)

| Was | Stand | Beleg |
|---|---|---|
| Beitreten (START Pad 2) | **funktioniert** | Labor A: „player 2 joined as 0xf9a400“, zweiter Jin im Bild |
| Tasten Spieler 2 | **funktioniert** | Quadrat/Dreieck/Kreuz kommen als 0x8000/0x1000/0x2000 im Datensatz an (wie Pad 1), Angriff bewegt die Figur (z 16000 → 16610) |
| Linker Stick Spieler 2 | **funktioniert** | Gleicher Ausschlag für beide Spieler (rechts, 3 s): beide laufen x +2700 / z −4500; danach läuft nur Spieler 2 (hoch) |
| Steuerkreuz | wie Spieler 1: bewegt im Spiel nicht (DualShock-2-Typ, 0x1021F0 übernimmt es dann nicht; auch Datensatz von Spieler 1 bleibt 0) | Log |
| SELECT-Sprung | **funktioniert** | Bild: Spieler 2 neben Spieler 1 |
| Verlassen / Wiedereinstieg | **funktioniert** | „Player 2 left“, Task-Kill 0x20B428, neuer Beitritt 0xf9b800 |
| Friendly Fire | gebaut, Treffer zwischen den Spielern ungetestet | – |
| Splitscreen-Prototyp (`T5_DW_SPLIT=1`) | **geteiltes Bild, Seiten richtig, HUD für Spieler 2** (Nutzertests 26.10.); Laufen, träge Kameras, Räume/Gegner bei Spieler 2, Sound, 16:9 nachgebessert, ungetestet | Nutzer |
| Spieltest im Fenster (Nutzer = Spieler 2, F4/`T5_PAD_P2`) | Beitritt im Fenster geklappt (Log 26.10.) | Log |

## Offen / bekannte Grenzen

- **Mehr als 2 Spieler (Nutzer 26.10.: später vielleicht):** Die PS2 hat nur zwei Pad-Ports (kein
  Multitap emuliert). Darum Spieler 3+ nicht über die Pad-Ports des Spiels, sondern ihre Eingaben
  vom Host direkt in den KI-Puffer ihrer Figur (gleicher Weg wie jetzt, nur Quelle Host statt
  0x2A98A0). Der Koop-Zustand (`State::coop`) soll dafür eine Liste von Spielern werden, die
  Ansichten (Splitscreen/Stream) N Kameras.

- **Kamera** folgt nur Spieler 1 (Kamera-Tasks folgen ManMgr-Eintrag +0); SELECT holt Spieler 2
  zurück ins Bild. **Geplant (Nutzer 26.10.):** lokal Splitscreen, online ohne Splitscreen (jeder
  sein Bild). Weg: zweite Kamera auf Spieler 2, abwechselnd gerendert (je Ansicht 30 fps, Logik
  60), das Fenster setzt die Hälften zusammen; online schickt der Host die Ansicht von Spieler 2
  als Bild-Stream, der Gast seine Eingaben. Befund: Die Kamera ist ein Einzelstück
  ([0x41BD10], 0x154308 legt keine zweite an), Update 0x154190 → 0x153BB0.
- **HUD** ohne Splitscreen nur für Spieler 1 (mit Splitscreen hat Spieler 2 seins).
- **Gegner**: Ziel = näherer Spieler (siehe Splitscreen), ungetestet.
- **Raumwechsel / Stage-Wechsel:** ob die Figur von Spieler 2 dabei entfernt wird, ist ungetestet;
  dann einfach wieder START.
- Der Tod von Spieler 2 zählt für das Spiel wie ein besiegter Gegner (mögliche Item-Drops).
- Nur Jin als Figur für Spieler 2 (andere Profile wären über den Become/Spawn-Ladeweg möglich).
- **Online-Koop** (coop-anything: Host-Autorität, Steam-Relay) ist nicht gebaut. Naheliegender
  Weg für diese Rekompilation: Eingaben von Spieler 2 übers Netz in Port 2 schreiben und das Bild
  streamen (Remote-Play-Prinzip), weil zwei getrennte Spielinstanzen deterministisch gleich laufen
  müssten (Lockstep), was für die Laufzeit nicht geprüft ist.
