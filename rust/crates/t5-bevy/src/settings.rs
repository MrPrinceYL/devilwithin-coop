//! Player settings: graphics (presets PS2 Classic / Modern / Custom, render scale, widescreen,
//! filtering, brightness), display (window mode, resolution, VSync, FPS counter, CD loading),
//! sound (master, music, effects) and the mouse camera. The options screen (`options.rs`) edits
//! them; `apply` puts them into effect.
//!
//! Saved as JSON next to the controls (`%APPDATA%\DevilWithinMod\settings.json`, or the
//! `T5_SETTINGS` path), so each player keeps their own. Every change applies and saves
//! immediately.
//!
//! The game runs at the console's 59.94 fields per second: its logic is frame-locked to
//! the vertical blank interrupt, as on the PlayStation 2, and each finished frame is shown at the
//! next retrace. VSync only decides whether the window waits for the monitor.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

/// Output volume as f32 bits (read by the audio callback).
pub static VOLUME: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0
/// The options screen is open (game input is held neutral).
pub static MENU_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum WindowKind {
    Windowed,
    Borderless,
    Fullscreen,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Preset {
    /// As the PlayStation 2 shows it: 640x448, 4:3, sharp pixels.
    Ps2Classic,
    /// 1440p-class rendering (3x), 16:9, smooth scaling.
    Modern,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Resource, Debug)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub preset: Preset,
    /// Render scale 1..=4 (1 = the console's 640x448, bit-exact).
    pub render_scale: u32,
    /// 16:9: the game's camera sees wider (devil_widescreen) and the picture fills 16:9.
    pub widescreen: bool,
    /// Bilinear filtering of the console picture (off = sharp pixels).
    pub smooth: bool,
    /// Display gamma applied to the final image (1.0 = as the console outputs it).
    pub gamma: f32,
    pub window: WindowKind,
    /// Window size (windowed) or video mode (fullscreen); None = 1280x960 window / the desktop's mode.
    pub resolution: Option<(u32, u32)>,
    pub vsync: bool,
    /// FPS counter in the top-right corner.
    pub show_fps: bool,
    /// DVD data reads this many times faster than the console (1 = original, 4, 8); streamed music and
    /// movies always keep the original speed. Off by default so the game stays 1:1.
    pub fast_load: u32,
    /// 0.0 ..= 1.5
    pub volume: f32,
    /// Streamed music (and movie sound) / sound effects, 0.0 ..= 1.5.
    pub music: f32,
    pub sfx: f32,
    /// Mouse camera: the cursor is captured while playing and moving the mouse turns the camera
    /// (right stick).
    pub mouse_camera: bool,
    pub invert_y: bool,
}

impl Default for Settings {
    /// Default settings.
    fn default() -> Self {
        Settings {
            version: 3,
            preset: Preset::Ps2Classic,
            render_scale: 1,
            widescreen: false,
            smooth: false,
            gamma: 1.0,
            window: WindowKind::Windowed,
            resolution: None,
            vsync: true,
            show_fps: true,
            fast_load: 1,
            volume: 1.0,
            music: 1.0,
            sfx: 1.0,
            mouse_camera: false,
            invert_y: false,
        }
    }
}

/// settings.json next to controls.json (or `T5_SETTINGS`).
pub fn settings_path() -> PathBuf {
    if let Some(p) = std::env::var_os("T5_SETTINGS") {
        return PathBuf::from(p);
    }
    crate::input::config_path().with_file_name("settings.json")
}

impl Settings {
    /// Load settings.json, or the defaults.
    pub fn load() -> Settings {
        let s: Settings = std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        s.sanitized()
    }
    /// Upgrade settings saved by older versions and clamp values into their valid ranges.
    pub fn sanitized(mut self) -> Settings {
        if self.version < 2 {
            // Version 2: the FPS counter is on by default.
            self.show_fps = true;
        }
        // Version 3: presets, render scale, widescreen, music / effects volume, mouse camera
        // (older files get those fields' defaults).
        self.version = 3;
        self.volume = self.volume.clamp(0.0, 1.5);
        self.music = self.music.clamp(0.0, 1.5);
        self.sfx = self.sfx.clamp(0.0, 1.5);
        self.gamma = self.gamma.clamp(0.5, 2.0);
        self.render_scale = self.render_scale.clamp(1, ps2_runtime::gpu::MAX_SCALE);
        if !matches!(self.fast_load, 1 | 4 | 8) {
            self.fast_load = 1;
        }
        self.preset = self.matching_preset();
        self
    }
    /// The preset whose picture settings these are (Custom if none).
    pub fn matching_preset(&self) -> Preset {
        for p in [Preset::Ps2Classic, Preset::Modern] {
            let mut t = self.clone();
            t.set_preset(p);
            if (t.render_scale, t.widescreen, t.smooth, t.gamma) == (self.render_scale, self.widescreen, self.smooth, self.gamma) {
                return p;
            }
        }
        Preset::Custom
    }
    /// Take a preset's picture settings (Custom keeps the current ones).
    pub fn set_preset(&mut self, p: Preset) {
        self.preset = p;
        match p {
            Preset::Ps2Classic => (self.render_scale, self.widescreen, self.smooth, self.gamma) = (1, false, false, 1.0),
            Preset::Modern => (self.render_scale, self.widescreen, self.smooth, self.gamma) = (3, true, true, 1.0),
            Preset::Custom => {}
        }
    }
    /// Write settings.json.
    pub fn save(&self) {
        let p = settings_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }
    /// Bevy present mode for the VSync setting.
    fn present_mode(&self) -> PresentMode {
        if self.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync }
    }
    /// The picture's aspect ratio (width / height).
    pub fn aspect(&self) -> f32 {
        if self.widescreen { 16.0 / 9.0 } else { 4.0 / 3.0 }
    }
}

/// Put the settings into effect: the emulator's volumes, render scale, loading speed and
/// widescreen, and the window (mode, size / video mode, VSync).
pub fn apply(s: &Settings, w: &mut Window, monitors: &[&Monitor]) {
    VOLUME.store(s.volume.to_bits(), Relaxed);
    ps2_runtime::spu2::MUSIC_VOLUME.store(s.music.to_bits(), Relaxed);
    ps2_runtime::spu2::SFX_VOLUME.store(s.sfx.to_bits(), Relaxed);
    ps2_runtime::gpu::RENDER_SCALE.store(s.render_scale, Relaxed);
    ps2_runtime::iop::fileio::FAST_LOAD.store(s.fast_load, Relaxed);
    t5_recomp::devil_widescreen::ENABLED.store(s.widescreen, Relaxed);
    let mode = match s.window {
        WindowKind::Windowed => WindowMode::Windowed,
        WindowKind::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        WindowKind::Fullscreen => {
            // The monitor's mode of that size with the highest refresh rate.
            let pick = s.resolution.and_then(|(rw, rh)| {
                monitors
                    .iter()
                    .flat_map(|m| m.video_modes.iter())
                    .filter(|v| v.physical_size == UVec2::new(rw, rh))
                    .max_by_key(|v| (v.refresh_rate_millihertz, v.bit_depth))
                    .cloned()
            });
            WindowMode::Fullscreen(MonitorSelection::Current, pick.map_or(VideoModeSelection::Current, VideoModeSelection::Specific))
        }
    };
    if w.mode != mode {
        w.mode = mode;
    }
    if s.window == WindowKind::Windowed {
        let (rw, rh) = s.resolution.unwrap_or((1280, 960));
        if (w.resolution.width() as u32, w.resolution.height() as u32) != (rw, rh) {
            w.resolution.set(rw as f32, rh as f32);
        }
    }
    let pm = s.present_mode();
    if w.present_mode != pm {
        w.present_mode = pm;
    }
}

/// Apply the saved settings at startup.
pub fn setup(settings: Res<Settings>, mut windows: Query<&mut Window, With<PrimaryWindow>>, monitors: Query<&Monitor>) {
    let m: Vec<&Monitor> = monitors.iter().collect();
    if let Ok(mut w) = windows.single_mut() {
        apply(&settings, &mut w, &m);
    }
}

/// Capture the cursor for the mouse camera while playing (options and the mod menu closed, the
/// window focused); free it otherwise.
pub fn mouse_capture(settings: Res<Settings>, mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>) {
    let Ok((w, mut c)) = windows.single_mut() else { return };
    let mod_menu = t5_recomp::devil_debug::MENU_TEXT.lock().map(|t| !t.is_empty()).unwrap_or(false);
    let in_menu = t5_recomp::devil_options::IN_MENU.load(Relaxed);
    let grab = settings.mouse_camera && w.focused && !MENU_OPEN.load(Relaxed) && !mod_menu && !in_menu;
    let (mode, visible) = if grab { (CursorGrabMode::Locked, false) } else { (CursorGrabMode::None, true) };
    if c.grab_mode != mode {
        c.grab_mode = mode;
    }
    if c.visible != visible {
        c.visible = visible;
    }
}

#[derive(Component)]
pub struct FpsText;

/// Build the FPS counter text (shown when enabled in the settings).
pub fn setup_fps(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(16.0), ..default() },
        TextColor(Color::srgb(1.0, 1.0, 0.4)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },
        Visibility::Hidden,
        FpsText,
    ));
}

/// Window counter for the FPS display: game frames (the console's frames actually produced,
/// i.e. the real game speed; 60 = full speed), averaged over ~0.5 s.
#[derive(Default)]
pub struct FpsWindow {
    start: Option<std::time::Instant>,
    game0: u64,
    flips0: u64,
    logic0: u64,
    text: String,
}

/// `game_frames`: frames sent to the screen (the console's ~60 Hz video rate); `flips`: frames
/// the game itself finished (display-buffer flips), i.e. how fast the game is actually running.
/// `game_frames`: pictures the game actually rendered (copy passes from a newly drawn frame);
/// `logic`: the game's logic frame counter (fights only; 0 elsewhere).
pub fn update_fps(game_frames: u64, logic: u64, flips: u64, settings: &Settings, w: &mut FpsWindow, q: &mut Query<(&mut Text, &mut Visibility), With<FpsText>>) {
    let Ok((mut t, mut vis)) = q.single_mut() else { return };
    *vis = if settings.show_fps { Visibility::Visible } else { Visibility::Hidden };
    let now = std::time::Instant::now();
    let start = *w.start.get_or_insert_with(|| {
        w.game0 = game_frames;
        w.flips0 = flips;
        w.logic0 = logic;
        now
    });
    let dt = now.duration_since(start).as_secs_f64();
    if dt >= 0.5 {
        let video = game_frames.saturating_sub(w.game0) as f64 / dt;
        let lg = logic.saturating_sub(w.logic0) as f64 / dt;
        w.text = if lg > 0.5 { format!("{video:.0} FPS (Logik {lg:.0})") } else { format!("{video:.0} FPS") };
        w.start = Some(now);
        w.game0 = game_frames;
        w.flips0 = flips;
        w.logic0 = logic;
    }
    if settings.show_fps && t.0 != w.text {
        t.0 = w.text.clone();
    }
}
