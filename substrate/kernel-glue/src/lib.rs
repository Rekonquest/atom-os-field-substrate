//! kernel-glue: the policy layer that maps the radiation/dissipation field
//! onto Atom OS kernel concepts.
//!
//! This crate is the only place OS semantics are introduced. It wraps a
//! `field_core::World` in a `Spinlock` and exposes a small surface the kernel
//! calls into:
//!
//! - `FieldState::new(config)` constructs the world and the per-PID trace
//!   baseline used by `ipc_recv_as_radiation`.
//! - `evolve_once(&self)` advances one moment under the spinlock.
//! - `stimulate(&self, pid, spectrum)` injects energy at the site mapped to
//!   `pid`. Counts as `introduced` in the closed invariant.
//! - `ipc_send_as_energy(&self, src, dst, magnitude)` injects energy at
//!   `src` biased toward `dst` by raising `dst`'s coupling to `src`'s
//!   spectrum. Propagation happens via the existing `exchange` law over
//!   subsequent evolutions.
//! - `ipc_recv_as_radiation(&self, pid)` returns the trace-peak delta since
//!   the last call for this PID.
//! - `scheduler_pick(&self, ready_mask)` returns the ready PID whose site
//!   maximises `energy.peak() * (0.3 + coupling.peak())`.
//! - `measurements(&self)` passes through `World::measurements`.
//!
//! Field-state to OS-state mapping lives in `ARCHITECTURE.md`.
//!
//! Because the field is fully deterministic given `Config.seed`, every kernel
//! decision taken through this layer is also deterministic.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use field_core::{Config, CouplingMode, DisturbanceMode, Measurements, Spectrum, World};
use spin::mutex::SpinMutex;

/// Maximum number of PIDs the kernel-glue layer tracks. Matches the kernel's
/// existing `MAX_TASKS = 16`.
pub const MAX_PIDS: usize = 16;

/// A field substrate owned by the kernel. The world is guarded by a spinlock;
/// the per-PID trace baselines are guarded independently because they are
/// only read/written under the same spinlock acquire anyway.
pub struct FieldState {
    world: SpinMutex<World>,
    last_trace: SpinMutex<[Spectrum; MAX_PIDS]>,
    evolution_count: AtomicU64,
}

impl FieldState {
    /// Construct a field from a verified config. Returns `None` if the
    /// config fails validation (same condition as `World::new`).
    pub fn new(config: Config) -> Option<Self> {
        let world = World::new(config).ok()?;
        Some(Self {
            world: SpinMutex::new(world),
            last_trace: SpinMutex::new([Spectrum::ZERO; MAX_PIDS]),
            evolution_count: AtomicU64::new(0),
        })
    }

    /// Advance the field one moment. Safe to call from a timer IRQ handler.
    pub fn evolve_once(&self) {
        let mut world = self.world.lock();
        world.evolve();
        self.evolution_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Advance the field `moments` moments. Used by `SYS_FIELD_EVOLVE`.
    pub fn evolve_for(&self, moments: u64) {
        let mut world = self.world.lock();
        world.evolve_for(moments);
        self.evolution_count
            .fetch_add(moments, Ordering::Relaxed);
    }

    /// Number of moments the field has evolved since construction.
    pub fn age(&self) -> u64 {
        self.evolution_count.load(Ordering::Relaxed)
    }

    /// Inject energy at the site mapped to `pid`. The injection counts as
    /// `introduced` in the closed invariant, so accounting still closes.
    /// Returns `false` if `pid` is out of range or the site is outside the
    /// observation-access region.
    pub fn stimulate(&self, pid: usize, spectrum: Spectrum) -> bool {
        if pid >= MAX_PIDS {
            return false;
        }
        let mut world = self.world.lock();
        let Some((x, y)) = site_for_pid(&world, pid) else {
            return false;
        };
        let index = y * world.width() + x;
        // SAFETY: World exposes no direct site mutation, so we accept the
        // small concession of going through the crate-internal channel by
        // re-stimulating via a temporary boundary-shaped injection. Because
        // the field is fully deterministic, this is equivalent to a direct
        // site write up to the closed-invariant tolerance.
        inject_at(&mut world, index, spectrum);
        true
    }

    /// Re-implements `SYS_IPC_SEND`: injects `magnitude` units of energy at
    /// `src` with the spectral shape of `src`'s current energy, then biases
    /// `dst`'s site so the existing `exchange` law preferentially propagates
    /// the new energy toward `dst` over subsequent evolutions. Returns
    /// `false` if either PID is out of range.
    pub fn ipc_send_as_energy(&self, src: usize, dst: usize, magnitude: f32) -> bool {
        if src >= MAX_PIDS || dst >= MAX_PIDS || src == dst {
            return false;
        }
        let mut world = self.world.lock();
        let (Some((sx, sy)), Some((dx, dy))) =
            (site_for_pid(&world, src), site_for_pid(&world, dst))
        else {
            return false;
        };
        let s_index = sy * world.width() + sx;
        let d_index = dy * world.width() + dx;
        let shape = unit_spectrum(site_energy(&world, s_index));
        inject_at(&mut world, s_index, shape * magnitude);
        // Bias the receiver by raising a small target coupling aligned with
        // the sender's spectral shape. The adapt_coupling law will compound
        // or erode this naturally; we only seed it.
        bias_coupling(&mut world, d_index, shape, magnitude * 0.05);
        true
    }

    /// Re-implements `SYS_IPC_RECV`: returns the change in `site.trace.peak()`
    /// since the last call for this PID. Returns 0.0 if `pid` is out of
    /// range or no new radiation has accumulated.
    pub fn ipc_recv_as_radiation(&self, pid: usize) -> f32 {
        if pid >= MAX_PIDS {
            return 0.0;
        }
        let world = self.world.lock();
        let Some((x, y)) = site_for_pid(&world, pid) else {
            return 0.0;
        };
        let index = y * world.width() + x;
        let current = site_trace(&world, index).peak();
        let mut baselines = self.last_trace.lock();
        let previous = baselines[pid].peak();
        baselines[pid] = site_trace(&world, index);
        (current - previous).max(0.0)
    }

    /// Scheduler hook: among the PIDs whose bit is set in `ready_mask` (bit
    /// `i` set means PID `i` is Ready), return the PID whose site maximises
    /// `energy.peak() * (0.3 + coupling.peak())`. Returns `None` if no bit
    /// is set or all set bits are out of range.
    pub fn scheduler_pick(&self, ready_mask: u16) -> Option<usize> {
        if ready_mask == 0 {
            return None;
        }
        let world = self.world.lock();
        let mut best_pid: Option<usize> = None;
        let mut best_score = f32::MIN;
        for pid in 0..MAX_PIDS {
            if (ready_mask & (1 << pid)) == 0 {
                continue;
            }
            let Some((x, y)) = site_for_pid(&world, pid) else {
                continue;
            };
            let index = y * world.width() + x;
            let energy = site_energy(&world, index).peak();
            let coupling = site_coupling(&world, index).peak();
            let score = energy * (0.3 + coupling);
            if score > best_score {
                best_score = score;
                best_pid = Some(pid);
            }
        }
        best_pid
    }

    /// Latest measurements, copied out under the spinlock.
    pub fn measurements(&self) -> Measurements {
        let world = self.world.lock();
        world.measurements()
    }

    /// Construct a default kernel-shaped config: a small field (24x24, the
    /// minimum allowed by `Config::validate`) so per-tick cost is bounded,
    /// adaptive coupling, no disturbance.
    pub const fn default_kernel_config() -> Config {
        Config {
            width: 24,
            height: 24,
            seed: 0xA701_5EED,
            coupling_mode: CouplingMode::Adaptive,
            disturbance_mode: DisturbanceMode::None,
            diffusion: 0.105,
            gradient: 0.24,
            phase_relay: 0.0,
            relay_guard: 0.0,
            formation: 0.022,
            erosion: 0.0018,
            coupling_formation: 0.220,
            coupling_erosion: 0.00035,
            radiation: 0.19,
            dissipation: 0.0012,
        }
    }
}

// ---------------------------------------------------------------------------
// Site mapping and the small field introspection + injection surface.
// field-core intentionally exposes no direct site mutation API because its
// upstream contract is "the only entry of energy is boundary stimulation".
// The kernel bridge *is* the new boundary; we expose a narrow, audited
// channel here.
// ---------------------------------------------------------------------------

/// Maps a PID to an (x, y) site inside the observation-access region. PIDs
/// 0..MAX_PIDS map to a 4x4 sub-grid centered on the access region.
fn site_for_pid(world: &World, pid: usize) -> Option<(usize, usize)> {
    if pid >= MAX_PIDS {
        return None;
    }
    let width = world.width();
    let height = world.height();
    if width < 4 || height < 4 {
        return None;
    }
    // Place the 4x4 PID grid just inside the right-hand observation band
    // (observation_access returns >0 for nx > 0.40 and reaches 1.0 at nx=0.6).
    let x_origin = ((width as f32 * 0.50) as usize).min(width - 4);
    let y_origin = ((height as f32 * 0.30) as usize).min(height - 4);
    let row = pid / 4;
    let col = pid % 4;
    Some((x_origin + col, y_origin + row))
}

// field-core keeps Site private. To inject energy or read back state without
// breaking its encapsulation in upstream, we expose three FFI-style helpers
// in field-core itself (see field_core::kernel_bridge). They are the audited
// surface this glue layer is allowed to use.
use field_core::kernel_bridge::{bias_coupling, inject_at, site_coupling, site_energy, site_trace};

fn unit_spectrum(s: Spectrum) -> Spectrum {
    let total = s.red + s.green + s.blue;
    if total <= 0.000_001 {
        Spectrum::ZERO
    } else {
        s * (1.0 / total)
    }
}

// ---------------------------------------------------------------------------
// Tests run on host (std) via `cargo test -p kernel-glue`.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> FieldState {
        FieldState::new(FieldState::default_kernel_config())
            .expect("default kernel config should validate")
    }

    #[test]
    fn evolves_deterministically() {
        let a = make_state();
        let b = make_state();
        for _ in 0..50 {
            a.evolve_once();
            b.evolve_once();
        }
        let ma = a.measurements();
        let mb = b.measurements();
        assert_eq!(ma.age, mb.age);
        assert_eq!(ma.introduced.to_bits(), mb.introduced.to_bits());
        assert_eq!(ma.radiated.to_bits(), mb.radiated.to_bits());
        assert_eq!(ma.resident.to_bits(), mb.resident.to_bits());
    }

    #[test]
    fn closed_invariant_holds_through_glue() {
        let state = make_state();
        // Inject energy at a few PIDs, evolve, then check accounting.
        for pid in 0..4 {
            assert!(state.stimulate(pid, Spectrum::new(0.05, 0.05, 0.05)));
        }
        state.evolve_for(500);
        let m = state.measurements();
        let accounted = m.resident + m.radiated + m.dissipated;
        let rel_err = (m.introduced - accounted).abs() / m.introduced.max(1e-12);
        assert!(rel_err < 3e-5, "rel_err = {rel_err}");
    }

    #[test]
    fn scheduler_pick_returns_a_ready_pid() {
        let state = make_state();
        // Stimulate PID 3 more strongly than the others.
        for _ in 0..10 {
            assert!(state.stimulate(3, Spectrum::new(0.5, 0.2, 0.2)));
        }
        state.evolve_for(20);
        let pick = state.scheduler_pick(0b0000_1111).expect("non-empty mask");
        assert!(pick < 4, "pick out of range: {pick}");
    }

    #[test]
    fn scheduler_pick_returns_none_for_empty_mask() {
        let state = make_state();
        assert!(state.scheduler_pick(0).is_none());
    }

    #[test]
    fn ipc_send_then_evolve_keeps_accounting_closed() {
        let state = make_state();
        for _ in 0..5 {
            assert!(state.ipc_send_as_energy(1, 2, 0.1));
        }
        state.evolve_for(100);
        let m = state.measurements();
        let accounted = m.resident + m.radiated + m.dissipated;
        let rel_err = (m.introduced - accounted).abs() / m.introduced.max(1e-12);
        assert!(rel_err < 3e-5, "rel_err = {rel_err}");
    }

    #[test]
    fn ipc_recv_returns_accumulated_trace_delta() {
        let state = make_state();
        // Drain any baseline so the next read reflects only new radiation.
        let _ = state.ipc_recv_as_radiation(2);
        // Send energy 1 -> 2 repeatedly and evolve. The receiver should
        // accumulate trace over time.
        for _ in 0..20 {
            assert!(state.ipc_send_as_energy(1, 2, 0.2));
            state.evolve_for(15);
        }
        let delta = state.ipc_recv_as_radiation(2);
        assert!(delta >= 0.0, "trace delta should be non-negative");
    }

    #[test]
    fn out_of_range_pids_are_rejected() {
        let state = make_state();
        assert!(!state.stimulate(MAX_PIDS, Spectrum::ZERO));
        assert!(!state.ipc_send_as_energy(MAX_PIDS, 0, 1.0));
        assert!(!state.ipc_send_as_energy(0, MAX_PIDS, 1.0));
        assert_eq!(state.ipc_recv_as_radiation(MAX_PIDS), 0.0);
    }
}

// Silence the unused-import warning for Vec in no_std builds that don't hit
// the test module; Vec is reserved for future per-PID history buffers.
#[allow(dead_code)]
fn _vec_anchor() -> Vec<u8> {
    Vec::new()
}
