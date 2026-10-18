//! 16:9 for Devil Within (a front-end option, not 1:1): the game's camera sees 4/3 wider while
//! its vertical view stays, and the front end shows the 640x448 picture at 16:9.
//!
//! The cameras are an array at 0x2ACDD0 (0x1B0 bytes each, count at 0x2ACD38): +0xF4 field of
//! view (an angle, 0x10000 = 360°, horizontal), +0x108 aspect (640/448). 0x101850 copies the
//! active one to 0x2AD5D4 / 0x2AD5D8 and 0x1019F0 builds the projection from them with
//! 0x21C8E0 (aspect, near, far, fov): m00 = 1 / tan(fov/2), m11 = aspect / tan(fov/2). The
//! game sets these fields only when it sets a camera up (0x101670, 0x101828), so the mod
//! rewrites them after every change: tan(fov'/2) = tan(fov/2) · 4/3 and aspect' = aspect · 4/3
//! give m00' = m00 · 3/4 and m11' = m11. The view-volume culling 0x10F900 gets its horizontal
//! angle and aspect from the constants 0x41ACE0 (65°) / 0x41ACE4 (4/3): widened the same way so
//! nothing at the new edges is culled.

use crate::devil_mod::word;
use ps2_runtime::{bus::ram_write, Machine};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// Widescreen on (set by the front end).
pub static ENABLED: AtomicBool = AtomicBool::new(false);

const CAMERAS: u32 = 0x002A_CDD0;
const CAMERA: u32 = 0x1B0;
const CAMERA_COUNT: u32 = 0x002A_CD38;
const FOV: u32 = 0xF4;
const ASPECT: u32 = 0x108;
const CULL_FOV: u32 = 0x0041_ACE0;
const CULL_ASPECT: u32 = 0x0041_ACE4;
const WIDER: f32 = 4.0 / 3.0;

/// Per camera field (fov, aspect): the game's value and what the mod wrote, so a change by the
/// game (it sets the fov and the aspect separately) is told apart from the mod's own value.
#[derive(Clone, Copy, Default)]
struct Field {
    orig: u32,
    patched: u32,
    active: bool,
}

#[derive(Clone, Copy, Default)]
struct Cam {
    fov: Field,
    aspect: Field,
}

struct State {
    cams: Vec<Cam>,
    /// The culling constants as the program has them (None until read).
    cull: Option<(u32, u32)>,
    on: bool,
}

static STATE: std::sync::Mutex<State> = std::sync::Mutex::new(State { cams: Vec::new(), cull: None, on: false });

/// The fov 4/3 wider: tan(fov'/2) = tan(fov/2) * 4/3 (angles: 0x10000 = 360°).
fn wider_fov(fov: u32) -> Option<u32> {
    if fov == 0 || fov >= 0x8000 {
        return None;
    }
    let tau = std::f64::consts::TAU;
    let half = fov as f64 / 65536.0 * tau / 2.0;
    let wide = 2.0 * (half.tan() * WIDER as f64).atan();
    Some((wide / tau * 65536.0).round() as u32)
}

fn wider_aspect(aspect: u32) -> Option<u32> {
    let a = f32::from_bits(aspect);
    (0.3..3.0).contains(&a).then(|| (a * WIDER).to_bits())
}

/// Keep field `f` (at `addr`, now `cur`) widened by `widen`, or put the game's value back.
fn keep(m: &mut Machine, f: &mut Field, addr: u32, cur: u32, on: bool, widen: fn(u32) -> Option<u32>) {
    if on {
        if f.active && cur == f.patched {
            return;
        }
        // A value the game (re)set: widen it.
        let Some(w) = widen(cur) else { return };
        *f = Field { orig: cur, patched: w, active: true };
        ram_write(m, addr, 4, w as u64);
    } else if f.active {
        if cur == f.patched {
            ram_write(m, addr, 4, f.orig as u64);
        }
        f.active = false;
    }
}

/// Every vblank: keep the cameras (and the culling) widened while widescreen is on, put the
/// game's values back when it goes off.
pub fn update(m: &mut Machine) {
    // T5_WIDE=1: widescreen for measurement runs (the front end sets it otherwise).
    static ENV: std::sync::Once = std::sync::Once::new();
    ENV.call_once(|| {
        if std::env::var_os("T5_WIDE").is_some() {
            ENABLED.store(true, Relaxed);
        }
    });
    let on = ENABLED.load(Relaxed);
    let Ok(mut st) = STATE.lock() else { return };
    // Culling constants (the program must be loaded: 65° and 4/3 there).
    if st.cull.is_none() {
        let (f, a) = (word(m, CULL_FOV), word(m, CULL_ASPECT));
        if f32::from_bits(f) == 65.0 && (f32::from_bits(a) - 4.0 / 3.0).abs() < 1e-4 {
            st.cull = Some((f, a));
        }
    }
    if let Some((f, a)) = st.cull {
        let (want_f, want_a) = if on {
            // Horizontal angle 4/3 wider, the vertical one (angle / aspect) kept.
            ((f32::from_bits(f) * WIDER).to_bits(), (f32::from_bits(a) * WIDER).to_bits())
        } else {
            (f, a)
        };
        if word(m, CULL_FOV) != want_f || word(m, CULL_ASPECT) != want_a {
            ram_write(m, CULL_FOV, 4, want_f as u64);
            ram_write(m, CULL_ASPECT, 4, want_a as u64);
        }
    }
    let n = word(m, CAMERA_COUNT).min(16) as usize;
    if st.cams.len() < n {
        st.cams.resize(n, Cam::default());
    }
    for i in 0..n {
        let base = CAMERAS + i as u32 * CAMERA;
        let (fov, aspect) = (word(m, base + FOV), word(m, base + ASPECT));
        let c = &mut st.cams[i];
        keep(m, &mut c.fov, base + FOV, fov, on, wider_fov);
        keep(m, &mut c.aspect, base + ASPECT, aspect, on, wider_aspect);
    }
    if on != st.on {
        st.on = on;
        ps2_runtime::diag::log(format!("devil widescreen: {}", if on { "16:9" } else { "4:3" }));
    }
}
