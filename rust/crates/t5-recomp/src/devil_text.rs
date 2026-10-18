//! Text changes of the mod (DevilWithinMod starts without Tekken 5), made in the game data:
//! before Devil Within is loaded (`kernel::IMAGE_PATCH`), the packs holding the texts are rebuilt
//! from the disc, laid over the image behind TK5DATA2.BIN (`Disc::overlay`, the file made longer
//! to reach them) and the program's file directory (0x362900) pointed at them. The game then
//! loads the changed texts itself, from the first frame on.
//!
//! The texts are "CAP" files (BDPK entry type 5, DevilWithin.md "Story-Texte"): "CAP\0", +0x18
//! index (8 bytes per entry {u16 text offset in 16-bit units, u16, u16 number, u16}), +0x1C texts
//! to the end of the file: 16-bit codes, every character code + 0xE1 (the
//! menu texts in capitals), 0 pause / end, 1 break (help texts), 2 line end, 3 space.

use ps2_runtime::Machine;

/// (CAP entry, text, replacement); matched on the decoded text (line ends and spaces as ' ').
/// The main menu's "return to mode select" takes the pause menu's "return to devil within title"
/// (and its help text); the memory card messages name Devil Within.
const EDITS: &[(&str, &str, &str)] = &[
    ("cap_teg_msg_font_menutext", "return to mode select", "RETURN TO DEVIL WITHIN TITLE"),
    (
        "cap_teg_msg_font_account",
        "exit devil within and return to the mode select screen.",
        "Return to the DEVIL WITHIN title screen.",
    ),
    ("cap_teg_msg_font_save", "tekken 5", "DEVIL WITHIN"),
];

/// New texts: the options menu's names and values (menutext) and help texts (account,
/// devil_options). Word gaps use code 3 and a text ends with code 2, as the game's own; help
/// texts break into lines of at most HELP_LINE characters with code 1.
const HELP_LINE: usize = 58;

/// The packs with those texts (TK5DATA2 files).
const PACKS: &[&str] = &["common/base.pak", "common/boot_us.pak", "common/demo.pak", "common/inst.pak"];

/// Devil Within's file directory (tools/dw_extract.py): 0x146 entries of 16 bytes {name checksum,
/// name offset into the pool at 0x363D60, offset in TK5DATA2.BIN, size}.
const FILE_DIR: u32 = 0x0036_2900;
const FILE_NAMES: u32 = 0x0036_3D60;
const FILE_COUNT: u32 = 0x146;

fn decode(v: u16) -> char {
    match v {
        2 | 3 => ' ',
        v if v >= 0x100 => {
            // Matched without regard to case.
            let c = v as i32 - 0xE1;
            if (0x20..0x7F).contains(&c) { (c as u8 as char).to_ascii_lowercase() } else { '\u{1}' }
        }
        _ => '\u{1}',
    }
}

fn encode(c: char) -> u16 {
    c as u16 + 0xE1
}

fn rd32(b: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?) as usize)
}

/// The CAP with `from` replaced by `to` everywhere, index offsets moved along; None if `from`
/// does not occur.
fn rebuild(cap: &[u8], from: &str, to: &str) -> Option<Vec<u8>> {
    let (index, texts) = (rd32(cap, 0x18)?, rd32(cap, 0x1C)?);
    if cap.get(..3)? != b"CAP" || index > texts || texts > cap.len() {
        return None;
    }
    let codes: Vec<u16> = cap[texts..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let chars: Vec<char> = codes.iter().map(|&v| decode(v)).collect();
    let from: Vec<char> = from.to_ascii_lowercase().chars().collect();
    let mut out = Vec::with_capacity(codes.len());
    let mut moved = vec![0usize; codes.len() + 1];
    let (mut i, mut hits) = (0, 0);
    while i < codes.len() {
        if chars[i..].starts_with(&from) {
            // Spaces keep the code the original used (3 space or 2 line end).
            let space = (i..i + from.len()).find(|&k| chars[k] == ' ').map(|k| codes[k]).unwrap_or(3);
            for k in i..i + from.len() {
                moved[k] = out.len();
            }
            out.extend(to.chars().map(|c| if c == ' ' { space } else { encode(c) }));
            i += from.len();
            hits += 1;
        } else {
            moved[i] = out.len();
            out.push(codes[i]);
            i += 1;
        }
    }
    moved[codes.len()] = out.len();
    if hits == 0 {
        return None;
    }
    let mut new = cap[..texts].to_vec();
    for e in (index..texts.saturating_sub(7)).step_by(8) {
        let off = u16::from_le_bytes([new[e], new[e + 1]]) as usize;
        let to = *moved.get(off).unwrap_or(&off) as u16;
        new[e..e + 2].copy_from_slice(&to.to_le_bytes());
    }
    new.extend(out.iter().flat_map(|v| v.to_le_bytes()));
    Some(new)
}

/// Help texts as the game writes them: sentences start with a capital, PS2 / FPS / VSync too.
fn sentence_case(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut start = true;
    for c in t.chars() {
        out.push(if start && c.is_ascii_alphabetic() { c.to_ascii_uppercase() } else { c });
        if c.is_ascii_alphabetic() {
            start = false;
        } else if c == '.' {
            start = true;
        }
    }
    out.replace("ps2", "PS2").replace("fps", "FPS").replace("vsync", "VSync")
}

/// A text's codes: words separated by code 3, broken into lines of `line` characters (code 1),
/// ended by code 2.
fn codes(text: &str, line: usize) -> Vec<u16> {
    let mut out = Vec::new();
    let mut col = 0;
    for (i, w) in text.split(' ').enumerate() {
        if i > 0 {
            if col + 1 + w.len() > line {
                out.push(1);
                col = 0;
            } else {
                out.push(3);
                col += 1;
            }
        }
        out.extend(w.chars().map(encode));
        col += w.len();
    }
    out.push(2);
    out
}

/// The CAP with `text` added under the next free number (index entry before the texts, the
/// text at the end): (new CAP, number).
fn append(cap: &[u8], text: &[u16]) -> Option<(Vec<u8>, u32)> {
    let (count, index, texts) = (rd32(cap, 4)?, rd32(cap, 0x18)?, rd32(cap, 0x1C)?);
    if cap.get(..3)? != b"CAP" || index + 8 * count != texts || texts > cap.len() {
        return None;
    }
    let number = (0..count).map(|i| u16::from_le_bytes([cap[index + 8 * i + 4], cap[index + 8 * i + 5]]) as u32).max().unwrap_or(0) + 1;
    let at = (cap.len() - texts) / 2;
    let last = index + 8 * (count - 1);
    let mut entry = cap[last..last + 8].to_vec();
    entry[0..2].copy_from_slice(&(at as u16).to_le_bytes());
    entry[4..6].copy_from_slice(&(number as u16).to_le_bytes());
    let mut new = cap[..texts].to_vec();
    new.extend_from_slice(&entry);
    new.extend_from_slice(&cap[texts..]);
    new[4..8].copy_from_slice(&((count + 1) as u32).to_le_bytes());
    new[0x1C..0x20].copy_from_slice(&((texts + 8) as u32).to_le_bytes());
    new.extend(text.iter().flat_map(|v| v.to_le_bytes()));
    Some((new, number))
}

/// The BDPK pack with its CAP entries edited, or None if nothing changed. Entries stay in place
/// except that those behind a grown CAP move by its growth (rounded to 0x80, keeping the data's
/// alignment); the header's group ends +0x40..+0x4C (+0x48 = start of the texture data, where a
/// texture pack is cut after the upload, 0x10B098) move with them.
fn repack(pack: &[u8]) -> Option<Vec<u8>> {
    if pack.get(..4)? != b"BDPK" {
        return None;
    }
    let n = rd32(pack, 4)?;
    let mut ents = Vec::new();
    for i in 0..n {
        let e = 0x80 * (i + 1);
        let name_end = pack.get(e + 0x14..e + 0x80)?.iter().position(|&b| b == 0).unwrap_or(0x6C);
        let name = std::str::from_utf8(&pack[e + 0x14..e + 0x14 + name_end]).unwrap_or("");
        let (size, off) = (rd32(pack, e + 8)?, rd32(pack, e + 0x10)?);
        let mut data = pack.get(off..off + size)?.to_vec();
        if pack[e + 2] == 5 {
            for (_, from, to) in EDITS.iter().filter(|x| x.0 == name) {
                if let Some(new) = rebuild(&data, from, to) {
                    data = new;
                }
            }
            let (texts, line) = match name {
                "cap_teg_msg_font_menutext" => (crate::devil_options::menu_texts(), usize::MAX),
                "cap_teg_msg_font_account" => (crate::devil_options::help_texts(), HELP_LINE),
                _ => (Vec::new(), 0),
            };
            let mut got = Vec::new();
            for t in texts {
                // Menu texts are capitals, as the game's own.
                let shown = if line == usize::MAX { t.to_ascii_uppercase() } else { sentence_case(&t) };
                if let Some((new, n)) = append(&data, &codes(&shown, line)) {
                    data = new;
                    got.push((t, n));
                }
            }
            if !got.is_empty() {
                if let Ok(mut nums) = crate::devil_options::NUMBERS.lock() {
                    if name == "cap_teg_msg_font_menutext" { nums.0 = got } else { nums.1 = got }
                }
            }
        }
        ents.push((e, off, size, data));
    }
    if ents.iter().all(|(_, _, size, data)| data.len() == *size) {
        return None;
    }
    // shift(x): how far an old offset x moves = the growth of the entries ending at or before x.
    let growth = |size: usize, new: usize| (new.saturating_sub(size) + 0x7F) & !0x7F;
    let shift = |x: usize| -> usize {
        ents.iter().filter(|(_, off, size, _)| off + size <= x).map(|(_, _, size, data)| growth(*size, data.len())).sum()
    };
    let total: usize = ents.iter().map(|(_, _, size, data)| growth(*size, data.len())).sum();
    let mut out = vec![0u8; pack.len() + total];
    // Header and entry records, then every gap and entry at its moved place.
    let records = 0x80 * (n + 1);
    out[..records].copy_from_slice(&pack[..records]);
    let mut prev_end = records;
    for (e, off, size, data) in &ents {
        let to = off + shift(*off);
        let gap = &pack[prev_end.min(*off)..*off];
        let gap_at = to - gap.len();
        out[gap_at..to].copy_from_slice(gap);
        out[to..to + data.len()].copy_from_slice(data);
        out[e + 8..e + 12].copy_from_slice(&(data.len() as u32).to_le_bytes());
        out[e + 0x10..e + 0x14].copy_from_slice(&(to as u32).to_le_bytes());
        prev_end = prev_end.max(off + size);
    }
    let tail = &pack[prev_end..];
    let tail_at = out.len() - tail.len();
    out[tail_at..].copy_from_slice(tail);
    out[8..12].copy_from_slice(&((rd32(pack, 8)? + total) as u32).to_le_bytes());
    for h in (0x40..0x50).step_by(4) {
        let v = rd32(pack, h)?;
        if v > records && v <= pack.len() {
            out[h..h + 4].copy_from_slice(&((v + shift(v)) as u32).to_le_bytes());
        }
    }
    Some(out)
}

/// The file offset of `vaddr` in an ELF image (its PT_LOAD segments).
fn elf_offset(elf: &[u8], vaddr: u32) -> Option<usize> {
    let (phoff, phnum) = (rd32(elf, 28)?, u16::from_le_bytes(elf.get(44..46)?.try_into().ok()?) as usize);
    (0..phnum).map(|i| phoff + 32 * i).find_map(|p| {
        let (kind, off, va, filesz) = (rd32(elf, p)?, rd32(elf, p + 4)?, rd32(elf, p + 8)? as u32, rd32(elf, p + 16)? as u32);
        (kind == 1 && va <= vaddr && vaddr < va + filesz).then(|| off + (vaddr - va) as usize)
    })
}

/// `kernel::IMAGE_PATCH`: rebuild the text packs, lay them behind TK5DATA2.BIN, point Devil
/// Within's file directory (in `elf`, before it is loaded) at them.
pub fn patch_program(m: &mut Machine, elf: &mut Vec<u8>) {
    let Some(d) = m.disc.as_mut() else { return };
    let Some((lba, size)) = d.find("TK5DATA2.BIN") else { return };
    let base = lba as u64 * 2048;
    let mut next = (base + size as u64 + 2047) & !2047;
    let (Some(dir), Some(names)) = (elf_offset(elf, FILE_DIR), elf_offset(elf, FILE_NAMES)) else { return };
    for i in 0..FILE_COUNT as usize {
        let e = dir + 16 * i;
        let (Some(name_off), Some(off), Some(len)) = (rd32(elf, e + 4), rd32(elf, e + 8), rd32(elf, e + 12)) else { break };
        let name_at = names + name_off;
        let name_end = elf[name_at..].iter().position(|&b| b == 0).unwrap_or(0);
        let name = String::from_utf8_lossy(&elf[name_at..name_at + name_end]).to_string();
        if !PACKS.contains(&name.as_str()) {
            continue;
        }
        let mut pack = vec![0u8; len];
        if d.read_into(base + off as u64, &mut pack).is_err() {
            continue;
        }
        let Some(new) = repack(&pack) else { continue };
        let new_len = new.len();
        d.overlay(next, new);
        elf[e + 8..e + 12].copy_from_slice(&((next - base) as u32).to_le_bytes());
        elf[e + 12..e + 16].copy_from_slice(&(new_len as u32).to_le_bytes());
        ps2_runtime::diag::log(format!("devil text: {name} {len} -> {new_len} bytes, now at TK5DATA2 +{:#x}", next - base));
        next = (next + new_len as u64 + 2047) & !2047;
    }
    d.set_size("TK5DATA2.BIN", (next - base) as u32);
}
