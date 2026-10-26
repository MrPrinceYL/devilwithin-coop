# Koop-Modus (Devil Within Mod)

Lokaler Zwei-Spieler-Koop für Devil Within: Spieler 2 steigt mit dem zweiten Controller als
zweiter Jin ins laufende Spiel ein. Vorgehen nach der Anleitung
[coop-anything](https://github.com/cranebeaker/coop-anything) (Weg 2 „von null bauen“, Abschnitt
„Decomp or recomp“: bei einer statischen Rekompilation kommt Koop über die Mod-Hooks ins Spiel).
Code: `rust/crates/t5-recomp/src/devil_mod.rs`, Abschnitt „Co-op“ (`coop_update`, `pad_as_game`).

## Spielen

- Zwei Gamepads anschließen (`play.bat` / `play.sh`). Port 1 = Tastatur/Maus + das zuerst
  benutzte Gamepad, Port 2 = das als zweites benutzte Gamepad (`t5-bevy/src/input.rs`).
  Spielt Spieler 1 nur mit der Tastatur, landet ein einzelnes Gamepad noch auf Port 1 (siehe
  „Offen“).
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
| Spieltest mit zwei Gamepads | offen (Nutzer) | – |

## Offen / bekannte Grenzen

- **Kamera** folgt nur Spieler 1 (Kamera-Tasks folgen ManMgr-Eintrag +0). Kein geteilter
  Bildschirm; SELECT holt Spieler 2 zurück ins Bild.
- **HUD** zeigt nur Spieler 1. Lebensleiste für Spieler 2 fehlt (Overlay wäre möglich).
- **Gegner** zielen auf Spieler 1 (Gegner-Ziel = ManMgr-Eintrag); Spieler 2 wird nur nebenbei
  getroffen. Ob Gegner Spieler 2 angreifen, ist ungetestet.
- **Raumwechsel / Stage-Wechsel:** ob die Figur von Spieler 2 dabei entfernt wird, ist ungetestet;
  dann einfach wieder START.
- Der Tod von Spieler 2 zählt für das Spiel wie ein besiegter Gegner (mögliche Item-Drops).
- Nur Jin als Figur für Spieler 2 (andere Profile wären über den Become/Spawn-Ladeweg möglich).
- Tastatur für Spieler 1 + ein einzelnes Gamepad für Spieler 2: noch nicht, das Gamepad geht an
  Port 1 (`input.rs` müsste dafür eine Einstellung bekommen).
- **Online-Koop** (coop-anything: Host-Autorität, Steam-Relay) ist nicht gebaut. Naheliegender
  Weg für diese Rekompilation: Eingaben von Spieler 2 übers Netz in Port 2 schreiben und das Bild
  streamen (Remote-Play-Prinzip), weil zwei getrennte Spielinstanzen deterministisch gleich laufen
  müssten (Lockstep), was für die Laufzeit nicht geprüft ist.
