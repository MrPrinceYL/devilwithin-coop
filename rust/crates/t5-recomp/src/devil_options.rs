//! OPTIONS in Devil Within's own main menu (not 1:1): graphics, display, sound and controls set
//! in the game's menu, drawn and run by the game's own menu engine like its other entries.
//!
//! The main menu (MT_MAINMENU, object at *0x41D040) shows a list (header {entries, count, node
//! 0x24, 7, 1, next}; retail 0x2D2CA8 with 4 entries, 0x2D2E00 with 5 when STAGE SELECT is
//! unlocked). An entry is 0x40 bytes: +0 text number (CAP menutext), +4 help number (CAP
//! account), +8 handler on choosing (menu, -, entry), +0x10 handler after its value changed,
//! +0x14 handler when it gets the cursor, +0x24 the layout node of its row (asb_top_menu: 2..5 =
//! moji_bar_main_menu_01..04, 0x25 = _05; the layout has no more rows), +0x28 flags (8: the value
//! shows only after choosing), +0x2C value texts (u32 text numbers), +0x30 their count, +0x34
//! function returning the address of the byte holding the value's index. The engine shows the
//! value at the row's right and steps it with left / right itself (as the pause menu's CAMERA
//! TYPE / VIBRATION). List engine: 0x1D27F8 (menu, header) adds a list and builds its rows,
//! 0x1D2A70 removes one, 0x1D2ED8 (menu, row) sets the cursor, 0x1D2CC8 (menu, number) the help
//! text. Circle / Cross choose, Triangle cancels (it does nothing in the main menu itself).
//!
//! The mod points the retail headers at its own tables in RAM the game leaves unused (REGION,
//! below the program at 0x100000): NEW GAME, CONTINUE, (STAGE SELECT,) LOAD DATA, OPTIONS
//! (+ RETURN TO DEVIL WITHIN TITLE when there is room for it). OPTIONS opens pages of at most
//! five rows: OPTIONS (GRAPHICS, DISPLAY, SOUND, CONTROLS, BACK), GRAPHICS, DISPLAY, SOUND,
//! CONTROLS. Page changes run in the game's thread (`kernel::request_thread_call`): remove the
//! shown list, add the new one, set the cursor; Triangle (hidden from the game while a page is
//! open, `pad::MASK`) goes back. The handlers are mod functions (`kernel::register_mod_func`).
//! The values live in VALUES: the front end applies the game's changes (GAME_CHANGED) and keeps
//! VALUES in step with its settings; every vblank they are written to the bytes the game shows.

use crate::devil_mod::word;
use ps2_runtime::kernel::request_thread_call;
use ps2_runtime::{bus::ram_write, Machine};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::Mutex;

/// The main menu object while it exists.
const MAIN_MENU: u32 = 0x0041_D040;
/// Retail list headers: 4 entries (no STAGE SELECT) / 5 entries.
const LIST4: u32 = 0x002D_2CA8;
const LIST5: u32 = 0x002D_2E00;
const ENTRY: u32 = 0x40;
const ADD_LIST: u32 = 0x001D_27F8;
const REMOVE_LIST: u32 = 0x001D_2A70;
const SET_CURSOR: u32 = 0x001D_2ED8;
const SET_HELP: u32 = 0x001D_2CC8;
const ROWS: [u32; 5] = [2, 3, 4, 5, 0x25];

/// RAM below the program that Devil Within leaves unused (RAM dumps 18.10.).
const REGION: u32 = 0x000F_8000;
const MAIN4: u32 = REGION;
const MAIN5: u32 = REGION + 0x200;
/// Two page buffers (header + 5 entries), used in turn so the one shown is never rewritten.
const PAGE: [u32; 2] = [REGION + 0x400, REGION + 0x600];
const LISTS: u32 = REGION + 0x800;
const BYTES: u32 = REGION + 0xF00;

/// Mod function addresses (no program code there): choosing row k, value of option o changed,
/// option o gets the cursor, option o's value byte.
const FN: u32 = 0x01FF_F000;
const FN_CHOOSE: u32 = FN;
const FN_CHANGED: u32 = FN + 0x100;
const FN_FOCUS: u32 = FN + 0x200;
const FN_VALUE: u32 = FN + 0x300;

const TRIANGLE: u16 = 0x1000;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Main,
    Options,
    Graphics,
    Display,
    Sound,
    Controls,
}

/// The settings the menu shows, as indices into `values(o)`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(usize)]
pub enum Opt {
    Preset,
    Scale,
    Aspect,
    Filter,
    Brightness,
    Mode,
    Resolution,
    VSync,
    Fps,
    Disc,
    Master,
    Music,
    Sfx,
    MouseCamera,
    MouseSens,
    InvertY,
    Deadzone,
}
pub const OPTS: usize = 17;
const ALL: [Opt; OPTS] = [
    Opt::Preset,
    Opt::Scale,
    Opt::Aspect,
    Opt::Filter,
    Opt::Brightness,
    Opt::Mode,
    Opt::Resolution,
    Opt::VSync,
    Opt::Fps,
    Opt::Disc,
    Opt::Master,
    Opt::Music,
    Opt::Sfx,
    Opt::MouseCamera,
    Opt::MouseSens,
    Opt::InvertY,
    Opt::Deadzone,
];

/// Window sizes / video modes the RESOLUTION row offers after "desktop".
pub const RESOLUTIONS: [(u32, u32); 8] = [(960, 720), (1280, 720), (1280, 960), (1600, 900), (1600, 1200), (1920, 1080), (2560, 1440), (3840, 2160)];

/// A value's texts (lower case, as the game's CAP texts; the menu shows them in capitals).
pub fn values(o: Opt) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let steps = |from: u32, to: u32, by: u32| (from..=to).step_by(by as usize).map(|v| v.to_string()).collect::<Vec<_>>();
    match o {
        Opt::Preset => s(&["ps2 classic", "modern", "custom"]),
        Opt::Scale => s(&["native 448p", "2x 896p", "3x 1344p", "4x 1792p"]),
        Opt::Aspect => s(&["4:3", "16:9"]),
        Opt::Filter => s(&["sharp", "smooth"]),
        Opt::Brightness => steps(50, 200, 10),
        Opt::Mode => s(&["windowed", "borderless", "fullscreen"]),
        Opt::Resolution => std::iter::once("desktop".to_string()).chain(RESOLUTIONS.iter().map(|(w, h)| format!("{w}x{h}"))).collect(),
        Opt::VSync | Opt::Fps | Opt::MouseCamera | Opt::InvertY => s(&["off", "on"]),
        Opt::Disc => s(&["original", "fast", "very fast"]),
        Opt::Master | Opt::Music | Opt::Sfx => steps(0, 150, 10),
        Opt::MouseSens => steps(1, 10, 1),
        Opt::Deadzone => steps(10, 60, 5),
    }
}

fn label(o: Opt) -> &'static str {
    match o {
        Opt::Preset => "preset",
        Opt::Scale => "render resolution",
        Opt::Aspect => "aspect ratio",
        Opt::Filter => "filtering",
        Opt::Brightness => "brightness",
        Opt::Mode => "display mode",
        Opt::Resolution => "resolution",
        Opt::VSync => "vsync",
        Opt::Fps => "fps counter",
        Opt::Disc => "disc loading",
        Opt::Master => "master volume",
        Opt::Music => "music volume",
        Opt::Sfx => "effects volume",
        Opt::MouseCamera => "mouse camera",
        Opt::MouseSens => "mouse sensitivity",
        Opt::InvertY => "invert mouse",
        Opt::Deadzone => "stick dead zone",
    }
}

fn help(o: Opt) -> &'static str {
    match o {
        Opt::Preset => "ps2 classic: the console's picture. modern: 1440p class, 16:9 and smooth. changing a picture setting makes it custom.",
        Opt::Scale => "draw the game at a multiple of the ps2's resolution. native is exact; higher needs a stronger graphics card.",
        Opt::Aspect => "16:9 widens the camera to fill a widescreen display. 4:3 is the original picture.",
        Opt::Filter => "smooth scales the picture with filtering; sharp keeps hard pixel edges.",
        Opt::Brightness => "brightness of the picture. 100 is the console's.",
        Opt::Mode => "windowed, borderless fullscreen or fullscreen with its own video mode.",
        Opt::Resolution => "window size, or the video mode in fullscreen. borderless always uses the desktop.",
        Opt::VSync => "wait for the display's refresh: no tearing. the game always runs at 60 frames per second.",
        Opt::Fps => "show the frames per second in the top right corner.",
        Opt::Disc => "read the disc faster than the console. music and movies keep their speed.",
        Opt::Master => "overall volume.",
        Opt::Music => "volume of the music and the movies.",
        Opt::Sfx => "volume of the sound effects and voices.",
        Opt::MouseCamera => "the mouse turns the camera while playing; the cursor is captured.",
        Opt::MouseSens => "how fast the mouse turns the camera.",
        Opt::InvertY => "moving the mouse up looks down.",
        Opt::Deadzone => "stick movement below this is ignored.",
    }
}

/// A row of a page.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Value(Opt),
    Page(Page),
    Back,
    Keys,
    /// The main menu's OPTIONS.
    OpenOptions,
}

fn page_rows(p: Page) -> Vec<Row> {
    match p {
        Page::Main => vec![],
        Page::Options => vec![Row::Page(Page::Graphics), Row::Page(Page::Display), Row::Page(Page::Sound), Row::Page(Page::Controls), Row::Back],
        Page::Graphics => [Opt::Preset, Opt::Scale, Opt::Aspect, Opt::Filter, Opt::Brightness].map(Row::Value).to_vec(),
        Page::Display => [Opt::Mode, Opt::Resolution, Opt::VSync, Opt::Fps, Opt::Disc].map(Row::Value).to_vec(),
        Page::Sound => vec![Row::Value(Opt::Master), Row::Value(Opt::Music), Row::Value(Opt::Sfx), Row::Back],
        Page::Controls => vec![Row::Value(Opt::MouseCamera), Row::Value(Opt::MouseSens), Row::Value(Opt::InvertY), Row::Value(Opt::Deadzone), Row::Keys],
    }
}

fn row_text(r: Row) -> String {
    match r {
        Row::Value(o) => label(o).into(),
        Row::Page(p) => match p {
            Page::Graphics => "graphics",
            Page::Display => "display",
            Page::Sound => "sound",
            Page::Controls => "controls",
            _ => "options",
        }
        .into(),
        Row::Back => "back".into(),
        Row::Keys => "key bindings".into(),
        Row::OpenOptions => "options".into(),
    }
}

fn row_help(r: Row) -> String {
    match r {
        Row::Value(o) => help(o).into(),
        Row::Page(Page::Graphics) => "graphics preset, render resolution, aspect ratio, filtering and brightness.".into(),
        Row::Page(Page::Display) => "display mode, resolution, vsync, fps counter and disc loading.".into(),
        Row::Page(Page::Sound) => "master, music and effects volume.".into(),
        Row::Page(_) => "mouse camera, sensitivity, stick dead zone and key bindings.".into(),
        Row::Back => "back to the previous menu. the triangle button also goes back.".into(),
        Row::Keys => "change which keys, mouse and gamepad buttons do what.".into(),
        Row::OpenOptions => "graphics, display, sound and control settings.".into(),
    }
}

/// Every text the menu needs in CAP menutext / account (devil_text adds them).
pub fn menu_texts() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for p in [Page::Options, Page::Graphics, Page::Display, Page::Sound, Page::Controls] {
        v.extend(page_rows(p).into_iter().map(row_text));
    }
    v.push(row_text(Row::OpenOptions));
    for o in ALL {
        v.extend(values(o));
    }
    v.sort();
    v.dedup();
    v
}

pub fn help_texts() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for p in [Page::Options, Page::Graphics, Page::Display, Page::Sound, Page::Controls] {
        v.extend(page_rows(p).into_iter().map(row_help));
    }
    v.push(row_help(Row::OpenOptions));
    v.sort();
    v.dedup();
    v
}

/// Text numbers devil_text gave the texts: (menutext, account).
pub static NUMBERS: Mutex<(Vec<(String, u32)>, Vec<(String, u32)>)> = Mutex::new((Vec::new(), Vec::new()));

fn number(menu: bool, t: &str) -> u32 {
    let n = NUMBERS.lock().unwrap();
    let list = if menu { &n.0 } else { &n.1 };
    list.iter().find(|(s, _)| s == t).map_or(0, |(_, k)| *k)
}

/// The values (indices into `values`), shared with the front end.
pub static VALUES: Mutex<[u8; OPTS]> = Mutex::new([0; OPTS]);
/// Set when the player changed a value in the game's menu (the front end applies it).
pub static GAME_CHANGED: AtomicU32 = AtomicU32::new(0);
/// KEY BINDINGS chosen: the front end opens its binding screen (and clears this).
pub static KEYS_OPEN: AtomicBool = AtomicBool::new(false);
/// A menu of the game takes the pad (the main menu or an options page): the front end maps the
/// mouse to menu input.
pub static IN_MENU: AtomicBool = AtomicBool::new(false);

struct State {
    page: Page,
    /// Rows of the page shown (choosing handlers read them).
    rows: Vec<Row>,
    /// The list header shown and the buffer for the next page.
    shown: u32,
    next_buf: usize,
    /// The retail list the main menu came with, and its OPTIONS row.
    main_list: u32,
    options_row: u32,
    /// Where each page was entered from (back goes there, cursor on that row).
    stack: Vec<(Page, u32)>,
    pending: Option<(Page, u32, bool)>,
    menu: u32,
    prev_buttons: u16,
}

static STATE: Mutex<State> = Mutex::new(State {
    page: Page::Main,
    rows: Vec::new(),
    shown: 0,
    next_buf: 0,
    main_list: 0,
    options_row: 0,
    stack: Vec::new(),
    pending: None,
    menu: 0,
    prev_buttons: 0,
});

/// Register the handlers.
pub fn install() {
    for k in 0..5 {
        ps2_runtime::kernel::register_mod_func(FN_CHOOSE + 8 * k, on_choose);
    }
    for o in 0..OPTS as u32 {
        ps2_runtime::kernel::register_mod_func(FN_CHANGED + 8 * o, on_changed);
        ps2_runtime::kernel::register_mod_func(FN_FOCUS + 8 * o, on_focus);
        ps2_runtime::kernel::register_mod_func(FN_VALUE + 8 * o, on_value);
    }
}

fn set_help(c: &mut ps2_runtime::Ctx, menu: u32, help: u32) {
    if help != 0 {
        c.call_guest(SET_HELP, &[menu as u64, help as u64]);
    }
}

/// A row was chosen (Circle / Cross).
fn on_choose(c: &mut ps2_runtime::Ctx, addr: u32) {
    let k = (addr - FN_CHOOSE) / 8;
    let Ok(mut st) = STATE.lock() else { return };
    let row = if st.page == Page::Main { Some(Row::OpenOptions) } else { st.rows.get(k as usize).copied() };
    match row {
        Some(Row::OpenOptions) => st.pending = Some((Page::Options, 0, true)),
        Some(Row::Page(p)) => st.pending = Some((p, 0, true)),
        Some(Row::Back) => st.pending = back(&st),
        Some(Row::Keys) => KEYS_OPEN.store(true, Relaxed),
        _ => {}
    }
    c.set_gpr(2, 0);
}

fn back(st: &State) -> Option<(Page, u32, bool)> {
    st.stack.last().map(|&(p, row)| (p, row, false))
}

/// Option o's value byte changed (left / right).
fn on_changed(c: &mut ps2_runtime::Ctx, addr: u32) {
    let o = ((addr - FN_CHANGED) / 8) as usize;
    let menu = c.gpr32(4);
    let m = ps2_runtime::machine();
    let n = values(ALL[o]).len() as u8;
    let v = (crate::devil_mod::word(m, BYTES + o as u32 & !3) >> (8 * (o as u32 & 3))) as u8;
    if v < n {
        // Flag while holding the lock: the front end reads both together.
        if let Ok(mut vals) = VALUES.lock() {
            vals[o] = v;
            GAME_CHANGED.fetch_or(1 << o, Relaxed);
        }
    }
    set_help(c, menu, number(false, help(ALL[o])));
}

/// Option o got the cursor.
fn on_focus(c: &mut ps2_runtime::Ctx, addr: u32) {
    let o = ((addr - FN_FOCUS) / 8) as usize;
    let menu = c.gpr32(4);
    set_help(c, menu, number(false, help(ALL[o])));
}

/// The address of option o's value byte.
fn on_value(c: &mut ps2_runtime::Ctx, addr: u32) {
    let o = (addr - FN_VALUE) / 8;
    c.set_gpr(2, (BYTES + o) as u64);
}

fn write_entry(m: &mut Machine, at: u32, words: [u32; 16]) {
    for (i, w) in words.iter().enumerate() {
        ram_write(m, at + 4 * i as u32, 4, *w as u64);
    }
}

/// The entry of a row at `node`.
fn entry(r: Row, k: u32, node: u32) -> [u32; 16] {
    let text = number(true, &row_text(r));
    let helpn = number(false, &row_help(r));
    let mut e = [0u32; 16];
    e[0] = text;
    e[1] = helpn;
    e[9] = node;
    match r {
        Row::Value(o) => {
            let o = o as u32;
            e[4] = FN_CHANGED + 8 * o;
            e[5] = FN_FOCUS + 8 * o;
            e[11] = LISTS + 0x40 * o;
            e[12] = values(ALL[o as usize]).len() as u32;
            e[13] = FN_VALUE + 8 * o;
        }
        _ => e[2] = FN_CHOOSE + 8 * k,
    }
    e
}

/// Write a page's header and entries into buffer `b`; returns the header.
fn write_page(m: &mut Machine, p: Page, b: usize) -> u32 {
    let at = PAGE[b];
    let rows = page_rows(p);
    for (k, r) in rows.iter().enumerate() {
        write_entry(m, at + 0x20 + ENTRY * k as u32, entry(*r, k as u32, ROWS[k]));
    }
    // Header as the retail one: {entries, count, cursor node 0x24, 7, 1, next 0}.
    for (i, w) in [at + 0x20, rows.len() as u32, 0x24, 7, 1, 0].iter().enumerate() {
        ram_write(m, at + 4 * i as u32, 4, *w as u64);
    }
    at
}

/// The retail main list `list` with OPTIONS: 4 entries + OPTIONS + RETURN at row 0x25, or (5
/// entries, no room) OPTIONS in place of RETURN.
fn patch_main(m: &mut Machine, list: u32, table: u32) -> u32 {
    let (entries, count) = (word(m, list), word(m, list + 4));
    let copy = |m: &mut Machine, from: u32, to: u32| {
        for o in (0..ENTRY).step_by(4) {
            let v = word(m, from + o);
            ram_write(m, to + o, 4, v as u64);
        }
    };
    let keep = count.min(4) - if count >= 5 { 0 } else { 1 };
    // Entries before RETURN (the last one) stay.
    for i in 0..keep {
        copy(m, entries + i * ENTRY, table + i * ENTRY);
    }
    let opt_row = keep;
    write_entry(m, table + opt_row * ENTRY, entry(Row::OpenOptions, 0, ROWS[opt_row as usize]));
    let mut n = opt_row + 1;
    if count == 4 {
        copy(m, entries + 3 * ENTRY, table + 4 * ENTRY);
        ram_write(m, table + 4 * ENTRY + 0x24, 4, ROWS[4] as u64);
        n = 5;
    }
    ram_write(m, list, 4, table as u64);
    ram_write(m, list + 4, 4, n as u64);
    opt_row
}

/// Every vblank.
pub fn update(m: &mut Machine) {
    let Ok(mut st) = STATE.lock() else { return };
    // The texts must be there (devil_text) and the program loaded.
    if number(true, "options") == 0 {
        return;
    }
    // Retail lists get OPTIONS once (their entries are read when the menu builds its rows).
    for (list, table) in [(LIST4, MAIN4), (LIST5, MAIN5)] {
        let e = word(m, list);
        if e != table && (0x0020_0000..0x0040_0000).contains(&e) && matches!(word(m, list + 4), 4 | 5) {
            let row = patch_main(m, list, table);
            if list == LIST4 {
                st.options_row = row;
            }
            ps2_runtime::diag::log(format!("devil options: OPTIONS in the main menu list {list:#x}"));
        }
    }
    // Value texts and the shown values.
    for (o, opt) in ALL.iter().enumerate() {
        for (i, t) in values(*opt).iter().enumerate() {
            let n = number(true, t);
            if word(m, LISTS + 0x40 * o as u32 + 4 * i as u32) != n {
                ram_write(m, LISTS + 0x40 * o as u32 + 4 * i as u32, 4, n as u64);
            }
        }
    }
    let vals = *VALUES.lock().unwrap();
    for (o, v) in vals.iter().enumerate() {
        ram_write(m, BYTES + o as u32, 1, *v as u64);
    }
    let menu = word(m, MAIN_MENU);
    if menu == 0 {
        // The main menu is gone (a game starts, the title): pages closed.
        if st.page != Page::Main || st.menu != 0 {
            st.page = Page::Main;
            st.stack.clear();
            st.pending = None;
            st.menu = 0;
        }
        ps2_runtime::pad::MASK.store(0, Relaxed);
        IN_MENU.store(false, Relaxed);
        return;
    }
    if st.menu != menu {
        st.menu = menu;
        st.page = Page::Main;
        st.stack.clear();
        st.pending = None;
    }
    IN_MENU.store(true, Relaxed);
    // Triangle goes back from a page.
    let buttons = m.shared.pad.buttons[0].load(Relaxed);
    let new = buttons & !st.prev_buttons;
    st.prev_buttons = buttons;
    if st.page != Page::Main && new & TRIANGLE != 0 && st.pending.is_none() {
        st.pending = back(&st);
    }
    ps2_runtime::pad::MASK.store(if st.page != Page::Main { TRIANGLE } else { 0 }, Relaxed);
    let Some((to, cursor, forward)) = st.pending.take() else { return };
    // The list shown now.
    let from = if st.page == Page::Main {
        let l = if word(m, LIST5) == MAIN5 && word(m, menu + 0x134) == LIST5 { LIST5 } else { LIST4 };
        st.main_list = l;
        l
    } else {
        st.shown
    };
    let (target, cur) = if to == Page::Main {
        (st.main_list, cursor)
    } else {
        let b = st.next_buf;
        st.next_buf ^= 1;
        (write_page(m, to, b), cursor)
    };
    if forward {
        let row = if st.page == Page::Main { st.options_row } else { word(m, menu + 0x118) };
        let page = st.page;
        st.stack.push((page, row));
    } else {
        st.stack.pop();
    }
    st.rows = page_rows(to);
    st.page = to;
    st.shown = target;
    // The cursor first: the new list's rows are laid out around it.
    request_thread_call(REMOVE_LIST, &[menu as u64, from as u64]);
    request_thread_call(SET_CURSOR, &[menu as u64, cur as u64]);
    request_thread_call(ADD_LIST, &[menu as u64, target as u64]);
    ps2_runtime::diag::log(format!("devil options: page {to:?} (list {target:#x}, cursor {cur})"));
}
