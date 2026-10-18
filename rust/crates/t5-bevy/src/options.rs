//! The game's OPTIONS (t5_recomp::devil_options: graphics, display, sound and controls set in
//! Devil Within's own main menu) and the front end's side of them: `sync` applies what the
//! player changed there to the settings and keeps the menu's values in step; the KEY BINDINGS
//! screen (chosen in the game's CONTROLS page, or F2) rebinds every pad button.
//!
//! Key bindings: mouse (hover selects, click adds an input, right-click clears), keyboard
//! (arrows, Enter adds, Delete clears, Esc back), gamepad (D-pad, Cross adds, Square clears,
//! Circle / Triangle back). Every change is saved at once.

use crate::input::{Input, CONTROLS};
use crate::settings::{self, Preset, Settings, WindowKind, MENU_OPEN};
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryWindow};
use ps2_runtime::pad;
use std::sync::atomic::Ordering::Relaxed;
use t5_recomp::devil_options::{self as game, Opt, OPTS};

const ACCENT: Color = Color::srgb(0.78, 0.07, 0.10);
const PANEL: Color = Color::srgba(0.05, 0.05, 0.07, 0.97);
const ROW: Color = Color::srgba(1.0, 1.0, 1.0, 0.03);
const ROW_SEL: Color = Color::srgba(0.78, 0.07, 0.10, 0.45);
const TEXT: Color = Color::srgb(0.92, 0.92, 0.92);
const DIM: Color = Color::srgb(0.55, 0.55, 0.58);
const ROW_H: f32 = 34.0;

// ---- the game's options <-> settings ----

fn steps(v: f32, from: f32, by: f32, n: usize) -> u8 {
    (((v - from) / by).round().max(0.0) as usize).min(n - 1) as u8
}

/// The menu's value indices for `s` (devil_options::values order).
fn to_values(s: &Settings, input: &Input) -> [u8; OPTS] {
    let mut v = [0u8; OPTS];
    v[Opt::Preset as usize] = match s.preset {
        Preset::Ps2Classic => 0,
        Preset::Modern => 1,
        Preset::Custom => 2,
    };
    v[Opt::Scale as usize] = (s.render_scale - 1) as u8;
    v[Opt::Aspect as usize] = s.widescreen as u8;
    v[Opt::Filter as usize] = s.smooth as u8;
    v[Opt::Brightness as usize] = steps(s.gamma, 0.5, 0.1, 16);
    v[Opt::Mode as usize] = match s.window {
        WindowKind::Windowed => 0,
        WindowKind::Borderless => 1,
        WindowKind::Fullscreen => 2,
    };
    v[Opt::Resolution as usize] = s.resolution.and_then(|r| game::RESOLUTIONS.iter().position(|x| *x == r)).map_or(0, |i| i as u8 + 1);
    v[Opt::VSync as usize] = s.vsync as u8;
    v[Opt::Fps as usize] = s.show_fps as u8;
    v[Opt::Disc as usize] = match s.fast_load {
        4 => 1,
        8 => 2,
        _ => 0,
    };
    v[Opt::Master as usize] = steps(s.volume, 0.0, 0.1, 16);
    v[Opt::Music as usize] = steps(s.music, 0.0, 0.1, 16);
    v[Opt::Sfx as usize] = steps(s.sfx, 0.0, 0.1, 16);
    v[Opt::MouseCamera as usize] = s.mouse_camera as u8;
    v[Opt::MouseSens as usize] = steps(input.controls.mouse_sensitivity, 0.005, 0.005, 10);
    v[Opt::InvertY as usize] = s.invert_y as u8;
    v[Opt::Deadzone as usize] = steps(input.controls.deadzone, 0.10, 0.05, 11);
    v
}

/// Take value `x` of option `o` into the settings.
fn set_value(o: Opt, x: u8, s: &mut Settings, input: &mut Input) {
    let f = x as f32;
    match o {
        Opt::Preset => s.set_preset([Preset::Ps2Classic, Preset::Modern, Preset::Custom][x.min(2) as usize]),
        Opt::Scale => s.render_scale = x as u32 + 1,
        Opt::Aspect => s.widescreen = x != 0,
        Opt::Filter => s.smooth = x != 0,
        Opt::Brightness => s.gamma = 0.5 + 0.1 * f,
        Opt::Mode => s.window = [WindowKind::Windowed, WindowKind::Borderless, WindowKind::Fullscreen][x.min(2) as usize],
        Opt::Resolution => s.resolution = if x == 0 { None } else { game::RESOLUTIONS.get(x as usize - 1).copied() },
        Opt::VSync => s.vsync = x != 0,
        Opt::Fps => s.show_fps = x != 0,
        Opt::Disc => s.fast_load = [1, 4, 8][x.min(2) as usize],
        Opt::Master => s.volume = 0.1 * f,
        Opt::Music => s.music = 0.1 * f,
        Opt::Sfx => s.sfx = 0.1 * f,
        Opt::MouseCamera => s.mouse_camera = x != 0,
        Opt::MouseSens => {
            let dz = input.controls.deadzone;
            input.set_tuning(dz, 0.005 * (f + 1.0));
        }
        Opt::InvertY => s.invert_y = x != 0,
        Opt::Deadzone => {
            let ms = input.controls.mouse_sensitivity;
            input.set_tuning(0.10 + 0.05 * f, ms);
        }
    }
}

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

/// Apply what the player changed in the game's OPTIONS, then show the settings' values there.
pub fn sync(mut s: ResMut<Settings>, mut input: ResMut<Input>, mut windows: Query<&mut Window, With<PrimaryWindow>>, monitors: Query<&Monitor>) {
    let Ok(mut vals) = game::VALUES.lock() else { return };
    let changed = game::GAME_CHANGED.swap(0, Relaxed);
    if changed != 0 {
        let mut sv = s.clone();
        for (i, o) in ALL.iter().enumerate() {
            if changed & (1 << i) != 0 {
                set_value(*o, vals[i], &mut sv, &mut input);
            }
        }
        if changed & (1 << Opt::Preset as usize) == 0 {
            sv.preset = sv.matching_preset();
        }
        *s = sv.sanitized();
        s.save();
        let m: Vec<&Monitor> = monitors.iter().collect();
        if let Ok(mut w) = windows.single_mut() {
            settings::apply(&s, &mut w, &m);
        }
    }
    *vals = to_values(&s, &input);
}

// ---- key bindings screen ----

#[derive(Resource, Default)]
pub struct OptState {
    open: bool,
    sel: usize,
    capturing: bool,
    prev_pad: u16,
    repeat_at: f64,
    last_cursor: Option<Vec2>,
}

#[derive(Component)]
pub struct KeysRoot;
#[derive(Component)]
pub struct BindRow(usize);
#[derive(Component)]
pub struct BindValue(usize);
#[derive(Component)]
pub struct RowsBox;
#[derive(Component)]
pub struct FooterButton;

/// Rows: every control, then "reset to defaults".
fn row_count() -> usize {
    CONTROLS.len() + 1
}

fn text(s: &str, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
}

/// Build the (hidden) key bindings screen.
pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(10),
            Visibility::Hidden,
            KeysRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(900.0),
                    max_width: Val::Percent(94.0),
                    height: Val::Px(700.0),
                    max_height: Val::Percent(94.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(22.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(ACCENT),
            ))
            .with_children(|p| {
                p.spawn(text("KEY BINDINGS", 30.0, ACCENT));
                p.spawn((Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: Val::Px(3.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default(), RowsBox)).with_children(|list| {
                    for i in 0..row_count() {
                        let name = if i < CONTROLS.len() { CONTROLS[i] } else { "Reset all to defaults" };
                        list.spawn((
                            Button,
                            Node {
                                height: Val::Px(ROW_H),
                                min_height: Val::Px(ROW_H),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(ROW),
                            BindRow(i),
                        ))
                        .with_children(|r| {
                            r.spawn(text(name, 17.0, TEXT));
                            r.spawn((text("", 16.0, TEXT), BindValue(i)));
                        });
                    }
                });
                p.spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|f| {
                    f.spawn(text("Enter / click / Cross: add input   Delete / right-click / Square: clear   Esc / Circle: back", 14.0, DIM));
                    f.spawn((
                        Button,
                        Node { padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                        BackgroundColor(ROW),
                        BorderColor::all(DIM),
                        FooterButton,
                    ))
                    .with_children(|x| {
                        x.spawn(text("Back", 16.0, TEXT));
                    });
                });
            });
        });
}

fn set_open(st: &mut OptState, open: bool, input: &Input) {
    st.open = open;
    st.capturing = false;
    if open {
        st.sel = 0;
        st.prev_pad = input.menu_buttons;
    }
    MENU_OPEN.store(open, Relaxed);
}

/// Keyboard, gamepad and hotkeys (F11 / Alt+Enter: windowed / borderless).
#[allow(clippy::too_many_arguments)]
pub fn keys_and_pad(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(Entity, &Gamepad, Option<&Name>)>,
    time: Res<Time>,
    mut st: ResMut<OptState>,
    mut s: ResMut<Settings>,
    mut input: ResMut<Input>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
) {
    let hid_new = input.hid_presses();
    input.mouse_look = s.mouse_camera && !st.open && !game::IN_MENU.load(Relaxed);
    input.invert_y = s.invert_y;
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if keys.just_pressed(KeyCode::F11) || (alt && keys.just_pressed(KeyCode::Enter)) {
        s.window = if s.window == WindowKind::Windowed { WindowKind::Borderless } else { WindowKind::Windowed };
        s.resolution = None;
        s.save();
        let m: Vec<&Monitor> = monitors.iter().collect();
        if let Ok(mut w) = windows.single_mut() {
            settings::apply(&s, &mut w, &m);
        }
    }
    if !st.open {
        if game::KEYS_OPEN.swap(false, Relaxed) || keys.just_pressed(KeyCode::F2) {
            set_open(&mut st, true, &input);
        }
        return;
    }
    let n = row_count();
    if st.capturing {
        if keys.just_pressed(KeyCode::Escape) {
            st.capturing = false;
        } else if let Some(b) = input.capture(&keys, &mouse, &pads, &hid_new) {
            if st.sel < CONTROLS.len() {
                input.add_binding(CONTROLS[st.sel], b);
            }
            st.capturing = false;
            st.prev_pad = input.menu_buttons;
        }
        return;
    }
    let now = time.elapsed_secs_f64();
    let held = input.menu_buttons;
    let mut new = held & !st.prev_pad;
    let dirs = pad::UP | pad::DOWN;
    if new & dirs != 0 {
        st.repeat_at = now + 0.4;
    } else if held & dirs != 0 && now >= st.repeat_at {
        new |= held & dirs;
        st.repeat_at = now + 0.08;
    }
    st.prev_pad = held;
    let k = |c: KeyCode| keys.just_pressed(c);
    if k(KeyCode::Escape) || k(KeyCode::F2) || new & (pad::CIRCLE | pad::TRIANGLE) != 0 {
        set_open(&mut st, false, &input);
        return;
    }
    if k(KeyCode::ArrowUp) || k(KeyCode::KeyW) || new & pad::UP != 0 {
        st.sel = (st.sel + n - 1) % n;
    }
    if k(KeyCode::ArrowDown) || k(KeyCode::KeyS) || new & pad::DOWN != 0 {
        st.sel = (st.sel + 1) % n;
    }
    let ok = (k(KeyCode::Enter) && !alt) || k(KeyCode::Space) || new & pad::CROSS != 0;
    let clear = k(KeyCode::Delete) || k(KeyCode::Backspace) || new & pad::SQUARE != 0;
    if st.sel == CONTROLS.len() {
        if ok {
            input.reset_bindings();
        }
    } else if ok {
        st.capturing = true;
    } else if clear {
        input.clear_binding(CONTROLS[st.sel]);
    }
}

/// Mouse: hover selects, click adds an input (or resets), right-click clears.
pub fn mouse(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut st: ResMut<OptState>,
    mut input: ResMut<Input>,
    windows: Query<&Window, With<PrimaryWindow>>,
    rows_q: Query<(&Interaction, &BindRow)>,
    footer: Query<&Interaction, (Changed<Interaction>, With<FooterButton>)>,
) {
    if !st.open || st.capturing {
        return;
    }
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    let moved = cursor.is_some() && cursor != st.last_cursor;
    st.last_cursor = cursor;
    if footer.iter().any(|i| *i == Interaction::Pressed) {
        set_open(&mut st, false, &input);
        return;
    }
    for (i, r) in &rows_q {
        if *i == Interaction::Hovered && moved {
            st.sel = r.0;
        }
        if *i == Interaction::Pressed {
            st.sel = r.0;
            if r.0 == CONTROLS.len() {
                if mouse_buttons.just_pressed(MouseButton::Left) {
                    input.reset_bindings();
                }
            } else if mouse_buttons.just_pressed(MouseButton::Right) {
                input.clear_binding(CONTROLS[r.0]);
            } else if mouse_buttons.just_pressed(MouseButton::Left) {
                st.capturing = true;
            }
        }
    }
}

/// Show the current state.
pub fn draw(
    st: Res<OptState>,
    input: Res<Input>,
    mut root: Query<&mut Visibility, With<KeysRoot>>,
    mut rows_q: Query<(&BindRow, &mut BackgroundColor)>,
    mut values: Query<(&BindValue, &mut Text)>,
    mut scroll: Query<(&mut ScrollPosition, &ComputedNode), With<RowsBox>>,
) {
    let want = if st.open { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
    if !st.open {
        return;
    }
    for (r, mut bg) in &mut rows_q {
        let c = if r.0 == st.sel { ROW_SEL } else { ROW };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (v, mut t) in &mut values {
        let txt = if st.capturing && v.0 == st.sel {
            "Press an input...  (Esc cancels)".to_string()
        } else if v.0 < CONTROLS.len() {
            input.binding_text(CONTROLS[v.0])
        } else {
            "Enter".to_string()
        };
        if t.0 != txt {
            t.0 = txt;
        }
    }
    if let Ok((mut sp, cn)) = scroll.single_mut() {
        let view = cn.size().y * cn.inverse_scale_factor;
        let row = ROW_H + 3.0;
        let top = st.sel as f32 * row;
        let mut y = sp.0.y;
        if top < y {
            y = top;
        } else if top + row > y + view {
            y = top + row - view;
        }
        if (sp.0.y - y).abs() > 0.5 {
            sp.0.y = y;
        }
    }
}

/// UI scale follows the window height (the screens are laid out for 960 lines).
pub fn ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    if let Ok(w) = windows.single() {
        let s = (w.height() / 960.0).clamp(0.6, 2.5);
        if (scale.0 - s).abs() > 0.01 {
            scale.0 = s;
        }
    }
}
