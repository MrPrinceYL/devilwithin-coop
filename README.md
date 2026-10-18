# DevilWithinMod: Devil Within (from Tekken 5, SLUS-21059), recompiled and modded

Devil Within, the action mode on the Tekken 5 disc, statically recompiled to Rust and started
directly (no Tekken 5 menus; Tekken 5 itself, StarBlade and Arcade History are not part of this
project). On top of the recompiled game sit a mod menu (become any character, spawn enemies and
items, no clip, every stage and boss fight, …), the reconstructed developer menus and an overlay.
Everything that was found and built is documented in **`DevilWithin.md`**.

This project was split off from `Decomps/Tekken5Decomp` (the 1:1 recompilation of the whole disc,
branch `main`). The recompilation pipeline is the same, reduced to Devil Within; the mod parts
(`t5-recomp/src/devil_*.rs`, the runtime hooks) are deliberately not 1:1.

```
Tekken 5 (USA).iso
   │ tools/ps2_extract.py   (ISO9660 → files + disc_files.csv)
   │ tools/tk5data.py       (TK5DATA3.BIN: LZ decompression; entry 4 = Devil Within)
   │ tools/make_elfs.py     (devil.elf)
   │ tools/iop_modules.py   (the IRX modules Devil Within loads from IRXARC.BIN)
   ▼
tools/run_ghidra.sh devil  (+ IOP=1 for the modules)
   ▼
../DevilWithin/export/devil/, export_iop/<module>/
   │ tools/build_sheets.py [--iop]
   ▼
sheets/*.csv, sheets/instructions/devil.csv, sheets/memory/devil.csv, sheets/iop/...
   │ tools/gen_rust.py (+ gen_iop.py, gen_vu.py)
   ▼
rust/ (Bevy app)                                     src/raw_c/devil, src/units (C)
```

## Playing

Run **`play.bat`** (Windows) or **`play.sh`** (Linux). The first run builds everything (that
takes a while), later runs start in seconds. The Tekken 5 disc image is read from `T5_DISC`
(default `E:\Decompilation\Game\Tekken 5 (USA).iso`); the game starts directly in Devil Within.

- Keyboard and gamepads work at once. F2 rebinds controls (`%APPDATA%\DevilWithinMod\controls.json`).
  Default keyboard: arrows / WASD = D-pad, X or Space = Cross, C = Circle, Z = Square,
  V = Triangle, Q / E = L1 / R1, 1 / 3 = L2 / R2, F / G = L3 / R3, Enter = Start,
  Tab / Right Shift = Select.
- **Mod menu: L3 + R3** (keyboard F + G). While it is open the game gets no input; Up / Down
  entry, Left / Right choice, Circle run. See `DevilWithin.md`, section 2b.
- **Developer menus:** `T5_DW_DEBUG=title` (debug title menu with STAGE SELECT / SELECT DEVIL /
  EXTRA-DEBUG; Down past "back" lists Makino's test room and every boss fight) or
  `T5_DW_DEBUG=dev` (developer mode menu). Set it before `play.bat`
  (PowerShell: `$env:T5_DW_DEBUG = "title"`).
- P opens the settings (volume, window mode, gamma, VSync, picture, FPS counter, faster disc
  loading). F1 shows the debug overlay. Holding F3 fast-forwards. F11 / Alt+Enter: fullscreen.
- Memory cards are folders (`memcard\port0`, `memcard\port1`; `T5_MEMCARD` moves them).
- `rust\t5_trace.log` is the diagnostics log. If something goes wrong, send it.

## Rebuilding from the disc

Python 3, Ghidra 12.1.4 with ghidra-emotionengine-reloaded 2.1.38 (**patched by
`tools/fix_ee_sleigh.py`**), ghidra-cli and a Rust toolchain. The work folder is
`Decomps/DevilWithin` (`extracted/`, `ghidra/` project DEVILWITHIN, `export/`, `export_iop/`,
`devTextEtc/`); `Decomps/Tekken5` belongs to Tekken5Decomp and is not used.

```bat
set ISO=E:\Decompilation\Game\Tekken 5 (USA).iso
set W=E:\Decompilation\Decomps\DevilWithin
cd tools
python fix_ee_sleigh.py
python ps2_extract.py "%ISO%" %W%\extracted
python tk5data.py "%ISO%" %W%\extracted
python make_elfs.py %W%\extracted
python iop_modules.py %W%\extracted
set GHIDRA=E:\Decompilation\Tools\ghidra-cli\target\release\ghidra.exe
set T5_WORK=%W%
bash run_ghidra.sh devil
set IOP=1& bash run_ghidra.sh
python build_sheets.py && python build_sheets.py --iop && python gen_rust.py
```

The tools are reduced to Devil Within: `tk5data.py` unpacks only TK5DATA3 entry 4, `make_elfs.py`
writes only `devil.elf`, `iop_modules.py` reads only `IRXARC.BIN` (Sound_Device_Library d91c3f,
NU_File_Manager 2fdaaa, NU_SOUND fdc40d, NU-Stream f76dd9, at the addresses Tekken5Decomp gave
them). Devil Within's own archive: `python tools/dw_extract.py "%ISO%" %W%\extracted` (TK5DATA2).

## How Devil Within is started

On the disc, Tekken 5 (`SLUS_210.59`) starts Devil Within through its program loader
(TK5DATA4.BIN), which unpacks TK5DATA3 entry 4 (an LZ-compressed ELF) and jumps to it. Here the
runtime boots that ELF itself (`ps2_runtime::kernel::BOOT_PACKED`, set in `t5-recomp` `boot()`):
it unpacks the entry from the disc image (LZ format: a flag byte, its bits LSB first above a
sentinel bit; 1 = literal byte, 0 = big-endian u16 back-reference with an 11-bit distance and a
5-bit length; flag 0 ends) and starts it like ExecPS2. Devil Within reboots the IOP with
`IOPRP300.IMG` and loads its modules from `IRXARC.BIN` itself, so it needs nothing from Tekken 5.
Its "return to the mode select screen" (LoadExecPS2 of `SLUS_210.59`) starts Devil Within again.

## Sheets

| Sheet | Key | Contents |
|---|---|---|
| `meta.csv` | `key` | Game ID, boot path, memory map, toolchain |
| `variants.csv` | `variant` | The program (`devil`): source, entry, gp, range, counts |
| `functions.csv` | `variant,addr` | name, size, section, module, unit, status, owner, fan-in/out, signature, C path, notes |
| `units.csv`, `calls.csv` | | Work partitions, static call graph |
| `instructions/devil.csv` | `addr` | Every instruction: function, bytes, disassembly, delay-slot flag, raw p-code |
| `memory/devil.csv` | `addr` | The program image, 64 bytes per row |
| `strings.csv`, `globals.csv`, `sections.csv`, `registers.csv` | | Ghidra's data |
| `vu_microcode.csv`, `vu_blocks.csv` | | VU micro code pairs and blocks (with entry shapes) |
| `dw_files.csv` | `path` | The files of Devil Within's archive TK5DATA2.BIN |
| `iop/…` | `module,…` | The four IOP modules: code, images, imports / exports |

## Rust / Bevy (`rust/`)

| Crate | What |
|---|---|
| `ee-core`, `iop-core` | ABI of generated EE (R5900) and IOP (R3000A) code |
| `ps2-runtime` | The PS2: EE kernel, DMAC, INTC, timers, GIF, GS (GPU rasterizer), VIF, VU, IPU, SIF, IOP libraries, SPU2 |
| `recomp/devil` | **Generated.** One function per guest function |
| `recomp_iop/<module>`, `t5-vu*` | **Generated.** IOP modules, VU micro code |
| `t5-recomp` | Program tables, boot, `t5-headless`; the mod: `devil_debug.rs` (overlay, developer menus, stage selects), `devil_mod.rs` (mod menu), `devil_chars.rs` (generated by `tools/dw_char_packs.py`) |
| `t5-bevy` | Bevy app: picture, SPU2 audio, input, overlay |
