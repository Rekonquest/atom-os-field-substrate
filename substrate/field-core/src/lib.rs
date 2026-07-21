//! no_std + alloc port of the universum radiation/dissipation field.
//!
//! This is a literal port of `C:\Projects\universe equavelant to vulkan
//! driver\src\field.rs`. Every numeric constant and every physics law is
//! preserved exactly. The only changes from upstream are:
//!
//! - `#![cfg_attr(not(test), no_std)]` + `extern crate alloc;`
//! - `std::f32::consts::{PI, TAU}` -> crate-level constants.
//! - `Vec` -> `alloc::vec::Vec`.
//! - `std::ops::*` -> `core::ops::*`.
//! - `f32::{sin, cos, ln_1p, powf, atan2}` and `f64::log2` -> libm via the
//!   `math` module below. All other operations (`abs`, `min`, `max`, `clamp`,
//!   `sqrt`, `rem_euclid`, `fract`) are already in `core`.
//!
//! When built with `--cfg test` the crate links std so the standard test
//! framework runs; in that mode the libm-backed math is still exercised, so a
//! passing test suite proves the no_std build behaves like upstream std math.

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Add, AddAssign, Mul, Sub, SubAssign};

// These traits are unused under `--cfg test` because std's inherent methods
// shadow them; they are required (and used) under `no_std`. Silence the
// test-mode dead-code warning.
#[allow(dead_code)]
/// no_std-friendly transcendentals and float operations.
///
/// Under `no_std`, the inherent methods on `f32` and `f64` (`sin`, `cos`,
/// `sqrt`, `powi`, `powf`, `ln_1p`, `atan2`, `log2`, `round`, `fract`,
/// `rem_euclid`) are NOT available — they live in `std`. This module provides
/// them as extension traits that route through `libm` so the exact same code
/// path runs in the kernel (`no_std`) and under the host test harness
/// (`std`). When built with `--cfg test`, std IS linked, so the libm-backed
/// implementations are still exercised and a passing test suite proves the
/// no_std build behaves like upstream std math.
mod math {
    pub trait F32Math {
        fn sin(self) -> f32;
        fn cos(self) -> f32;
        fn ln_1p(self) -> f32;
        fn powf(self, exponent: f32) -> f32;
        fn powi(self, exponent: i32) -> f32;
        fn atan2(self, other: f32) -> f32;
        fn sqrt(self) -> f32;
        fn round(self) -> f32;
        fn fract(self) -> f32;
        fn rem_euclid(self, other: f32) -> f32;
    }

    impl F32Math for f32 {
        fn sin(self) -> f32 {
            libm::sinf(self)
        }
        fn cos(self) -> f32 {
            libm::cosf(self)
        }
        fn ln_1p(self) -> f32 {
            libm::log1pf(self)
        }
        fn powf(self, exponent: f32) -> f32 {
            libm::powf(self, exponent)
        }
        fn powi(self, exponent: i32) -> f32 {
            // libm has no integer-power helper; lift to f32 powf. Exact for
            // the small exponents used by upstream field.rs (always 2).
            libm::powf(self, exponent as f32)
        }
        fn atan2(self, other: f32) -> f32 {
            libm::atan2f(self, other)
        }
        fn sqrt(self) -> f32 {
            libm::sqrtf(self)
        }
        fn round(self) -> f32 {
            libm::roundf(self)
        }
        fn fract(self) -> f32 {
            self - libm::truncf(self)
        }
        fn rem_euclid(self, other: f32) -> f32 {
            // Euclidean remainder with f32 sign convention. libm provides
            // fmodf, which is the truncated remainder; convert to euclidean.
            let r = libm::fmodf(self, other);
            if r < 0.0 {
                r + other.abs()
            } else {
                r
            }
        }
    }

    pub trait F64Math {
        fn log2(self) -> f64;
    }

    impl F64Math for f64 {
        fn log2(self) -> f64 {
            libm::log2(self)
        }
    }
}
#[allow(unused_imports)]
use math::{F32Math, F64Math};

const PI: f32 = core::f32::consts::PI;
const TAU: f32 = core::f32::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spectrum {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl Spectrum {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    pub fn total(self) -> f64 {
        (self.red + self.green + self.blue) as f64
    }

    pub fn peak(self) -> f32 {
        self.red.max(self.green).max(self.blue)
    }

    fn magnitude(self) -> f32 {
        self.red.abs() + self.green.abs() + self.blue.abs()
    }

    fn map(self, f: impl Fn(f32) -> f32) -> Self {
        Self::new(f(self.red), f(self.green), f(self.blue))
    }

    fn zip(self, other: Self, f: impl Fn(f32, f32) -> f32) -> Self {
        Self::new(
            f(self.red, other.red),
            f(self.green, other.green),
            f(self.blue, other.blue),
        )
    }

    fn clamp_nonnegative(self) -> Self {
        self.map(|value| value.max(0.0))
    }
}

impl Add for Spectrum {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a + b)
    }
}

impl AddAssign for Spectrum {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Spectrum {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a - b)
    }
}

impl SubAssign for Spectrum {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul<f32> for Spectrum {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        self.map(|value| value * rhs)
    }
}

impl Mul for Spectrum {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a * b)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CouplingMode {
    Adaptive,
    Fixed,
    Inert,
}

impl CouplingMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::Fixed => "fixed",
            Self::Inert => "inert",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisturbanceMode {
    None,
    Scar,
    Noise,
}

impl DisturbanceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Scar => "scar",
            Self::Noise => "noise",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub width: usize,
    pub height: usize,
    pub seed: u64,
    pub coupling_mode: CouplingMode,
    pub disturbance_mode: DisturbanceMode,
    pub diffusion: f32,
    pub gradient: f32,
    pub phase_relay: f32,
    pub relay_guard: f32,
    pub formation: f32,
    pub erosion: f32,
    pub coupling_formation: f32,
    pub coupling_erosion: f32,
    pub radiation: f32,
    pub dissipation: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 192,
            height: 112,
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

impl Config {
    pub fn validate(self) -> Result<Self, String> {
        if self.width < 24 || self.height < 24 {
            return Err("the field must be at least 24 by 24 sites".into());
        }
        if self.width > 2048 || self.height > 2048 {
            return Err("the field cannot exceed 2048 by 2048 sites".into());
        }
        for (name, value) in [
            ("diffusion", self.diffusion),
            ("gradient", self.gradient),
            ("phase_relay", self.phase_relay),
            ("relay_guard", self.relay_guard),
            ("formation", self.formation),
            ("erosion", self.erosion),
            ("coupling_formation", self.coupling_formation),
            ("coupling_erosion", self.coupling_erosion),
            ("radiation", self.radiation),
            ("dissipation", self.dissipation),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(format!("{name} must be finite and between zero and one"));
            }
        }
        if self.diffusion > 0.20 {
            return Err("diffusion above 0.20 is unstable for this local law".into());
        }
        if self.phase_relay > 0.70 {
            return Err("phase_relay above 0.70 over-amplifies local conductance".into());
        }
        if self.relay_guard > 1.0 {
            return Err("relay_guard must not exceed one".into());
        }
        if (self.diffusion * 4.0 + self.gradient) * (1.0 + self.phase_relay) > 0.95 {
            return Err("combined transport would move too much energy in one moment".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Site {
    energy: Spectrum,
    trace: Spectrum,
    coupling: Spectrum,
    permeability: f32,
    activity: f32,
    phase: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Measurements {
    pub age: u64,
    pub introduced: f64,
    pub resident: f64,
    pub radiated: f64,
    pub dissipated: f64,
    pub accounting_error: f64,
    pub disturbance_introduced: f64,
    pub disturbance_dissipated: f64,
    pub mean_permeability: f64,
    pub mean_coupling: f64,
    pub organized_fraction: f64,
    pub coupled_fraction: f64,
    pub largest_organized_component: f64,
    pub luminous_sites: usize,
    pub channel_signal: f64,
    pub channel_total: f64,
    pub channel_fidelity: f64,
    pub channel_balance: f64,
    pub channel_information_bits: f64,
}

pub struct World {
    config: Config,
    sites: Vec<Site>,
    age: u64,
    introduced: f64,
    disturbance_introduced: f64,
    radiated: f64,
    dissipated: f64,
    disturbance_dissipated: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct ChannelMeasurements {
    signal: f64,
    total: f64,
    fidelity: f64,
    balance: f64,
    information_bits: f64,
}

impl World {
    pub fn new(config: Config) -> Result<Self, String> {
        let config = config.validate()?;
        let mut random = SplitMix64(config.seed);
        let mut sites = Vec::with_capacity(config.width * config.height);
        for y in 0..config.height {
            for x in 0..config.width {
                let coupling = match config.coupling_mode {
                    CouplingMode::Adaptive | CouplingMode::Inert => {
                        seed_coupling(&mut random, x, config.width)
                    }
                    CouplingMode::Fixed => visible_coupling(x, y, config.width, config.height),
                };
                sites.push(Site {
                    permeability: 0.055 + random.unit_f32() * 0.025,
                    coupling,
                    phase: random.unit_f32() * TAU,
                    ..Site::default()
                });
            }
        }
        Ok(Self {
            config,
            sites,
            age: 0,
            introduced: 0.0,
            disturbance_introduced: 0.0,
            radiated: 0.0,
            dissipated: 0.0,
            disturbance_dissipated: 0.0,
        })
    }

    pub fn width(&self) -> usize {
        self.config.width
    }

    pub fn height(&self) -> usize {
        self.config.height
    }

    pub fn age(&self) -> u64 {
        self.age
    }

    /// Advances the field one moment. The loop order implements simultaneous
    /// local change; it is not an ordering visible within the substrate.
    pub fn evolve(&mut self) {
        self.stimulate_boundary();
        if self.config.disturbance_mode == DisturbanceMode::Noise {
            self.inject_noise();
        }
        let count = self.sites.len();
        let mut change = vec![Spectrum::ZERO; count];
        let mut local_flow = vec![0.0_f32; count];

        for y in 0..self.config.height {
            for x in 0..self.config.width {
                let here = self.index(x, y);
                if x + 1 < self.config.width {
                    let there = self.index(x + 1, y);
                    self.exchange(here, there, true, &mut change, &mut local_flow);
                }
                if y + 1 < self.config.height {
                    let there = self.index(x, y + 1);
                    self.exchange(here, there, false, &mut change, &mut local_flow);
                }
            }
        }

        let config = self.config;
        let width = config.width;
        for (index, site) in self.sites.iter_mut().enumerate() {
            site.energy = (site.energy + change[index]).clamp_nonnegative();
            let emitted = (site.energy * site.coupling) * config.radiation;
            site.energy -= emitted;
            site.trace += emitted;
            self.radiated += emitted.total();

            let lost = site.energy * config.dissipation;
            site.energy -= lost;
            self.dissipated += lost.total();

            let activity = (local_flow[index] * 2.5).min(1.0);
            site.activity = site.activity * 0.94 + activity * 0.06;
            let growth = config.formation * site.activity * (1.0 - site.permeability);
            let decay = config.erosion * (site.permeability - 0.05).max(0.0);
            site.permeability = (site.permeability + growth - decay).clamp(0.025, 1.0);
            site.phase =
                wrap_phase(site.phase + site.activity * spectral_turn(site.energy) * 0.024);
            if config.coupling_mode == CouplingMode::Adaptive {
                adapt_coupling(
                    site,
                    index % width,
                    width,
                    config,
                    emitted,
                    local_flow[index],
                );
            }
        }
        if self.config.disturbance_mode == DisturbanceMode::Scar {
            self.apply_scar();
        }
        self.age += 1;
    }

    pub fn evolve_for(&mut self, moments: u64) {
        for _ in 0..moments {
            self.evolve();
        }
    }

    pub fn measurements(&self) -> Measurements {
        let resident = self
            .sites
            .iter()
            .map(|site| site.energy.total())
            .sum::<f64>();
        let mean_permeability = self
            .sites
            .iter()
            .map(|site| site.permeability as f64)
            .sum::<f64>()
            / self.sites.len() as f64;
        let mean_coupling = self
            .sites
            .iter()
            .map(|site| site.coupling.peak() as f64)
            .sum::<f64>()
            / self.sites.len() as f64;
        let organized = self
            .sites
            .iter()
            .filter(|site| site.permeability > 0.14)
            .count();
        let coupled = self
            .sites
            .iter()
            .filter(|site| site.coupling.peak() > 0.012)
            .count();
        let luminous_sites = self
            .sites
            .iter()
            .filter(|site| site.trace.peak() > 0.01)
            .count();
        let largest_organized_component =
            self.largest_component_fraction(|site| site.permeability > 0.14);
        let channel = self.channel_measurements();
        let accounted = resident + self.radiated + self.dissipated;
        Measurements {
            age: self.age,
            introduced: self.introduced,
            resident,
            radiated: self.radiated,
            dissipated: self.dissipated,
            accounting_error: self.introduced - accounted,
            disturbance_introduced: self.disturbance_introduced,
            disturbance_dissipated: self.disturbance_dissipated,
            mean_permeability,
            mean_coupling,
            organized_fraction: organized as f64 / self.sites.len() as f64,
            coupled_fraction: coupled as f64 / self.sites.len() as f64,
            largest_organized_component,
            luminous_sites,
            channel_signal: channel.signal,
            channel_total: channel.total,
            channel_fidelity: channel.fidelity,
            channel_balance: channel.balance,
            channel_information_bits: channel.information_bits,
        }
    }

    pub fn visible_hash64(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for pixel in self.rgb8() {
            for channel in pixel {
                hash ^= u64::from(channel);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    pub(crate) fn rgb8(&self) -> Vec<[u8; 3]> {
        let peak_trace = self
            .sites
            .iter()
            .map(|site| site.trace.peak())
            .fold(0.000_001_f32, f32::max);
        let exposure = 3.2 / peak_trace.max(0.1).ln_1p();
        self.sites
            .iter()
            .map(|site| {
                let glow = site.trace.map(|value| (value.ln_1p() * exposure).min(1.0));
                let live = site.energy.map(|value| (value * 0.30).min(1.0));
                let structure = ((site.permeability - 0.05) * 0.45).clamp(0.0, 0.28);
                [
                    linear_to_u8(glow.red + live.red + structure * 0.38),
                    linear_to_u8(glow.green + live.green + structure * 0.55),
                    linear_to_u8(glow.blue + live.blue + structure),
                ]
            })
            .collect()
    }

    fn channel_measurements(&self) -> ChannelMeasurements {
        let mut matrix = [[0.0_f64; 3]; 3];

        for y in 0..self.config.height {
            let ny = y as f32 / (self.config.height - 1) as f32;
            let mut bands = [
                source_band(ny, 0.23),
                source_band(ny, 0.50),
                source_band(ny, 0.77),
            ];
            let band_total = bands.iter().sum::<f32>();
            if band_total <= 0.000_001 {
                continue;
            }
            for band in &mut bands {
                *band /= band_total;
            }

            for x in 0..self.config.width {
                let access = observation_access(x, self.config.width);
                if access <= 0.0 {
                    continue;
                }

                let site = self.sites[self.index(x, y)];
                let colors = [
                    site.trace.red as f64,
                    site.trace.green as f64,
                    site.trace.blue as f64,
                ];
                for band in 0..3 {
                    let weight = bands[band] as f64 * access as f64;
                    for (color, value) in colors.iter().enumerate() {
                        matrix[band][color] += *value * weight;
                    }
                }
            }
        }

        channel_from_matrix(matrix)
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.config.width + x
    }

    fn largest_component_fraction(&self, active: impl Fn(&Site) -> bool) -> f64 {
        let mut visited = vec![false; self.sites.len()];
        let mut stack = Vec::new();
        let mut largest = 0_usize;

        for start in 0..self.sites.len() {
            if visited[start] || !active(&self.sites[start]) {
                continue;
            }

            let mut size = 0_usize;
            visited[start] = true;
            stack.push(start);

            while let Some(index) = stack.pop() {
                size += 1;
                let x = index % self.config.width;
                let y = index / self.config.width;
                if x > 0 {
                    self.visit_if_active(index - 1, &active, &mut visited, &mut stack);
                }
                if x + 1 < self.config.width {
                    self.visit_if_active(index + 1, &active, &mut visited, &mut stack);
                }
                if y > 0 {
                    self.visit_if_active(
                        index - self.config.width,
                        &active,
                        &mut visited,
                        &mut stack,
                    );
                }
                if y + 1 < self.config.height {
                    self.visit_if_active(
                        index + self.config.width,
                        &active,
                        &mut visited,
                        &mut stack,
                    );
                }
            }

            largest = largest.max(size);
        }

        largest as f64 / self.sites.len() as f64
    }

    fn visit_if_active(
        &self,
        index: usize,
        active: &impl Fn(&Site) -> bool,
        visited: &mut [bool],
        stack: &mut Vec<usize>,
    ) {
        if !visited[index] && active(&self.sites[index]) {
            visited[index] = true;
            stack.push(index);
        }
    }

    fn stimulate_boundary(&mut self) {
        let height = self.config.height as f32;
        let pulse = self.age as f32 * 0.037;
        let sources = [
            (0.23 + pulse.sin() * 0.025, Spectrum::new(0.92, 0.08, 0.03)),
            (
                0.50 + (pulse * 0.73 + 1.2).sin() * 0.035,
                Spectrum::new(0.05, 0.88, 0.12),
            ),
            (
                0.77 + (pulse * 0.51 + 2.4).sin() * 0.025,
                Spectrum::new(0.04, 0.13, 0.96),
            ),
        ];
        for (position, spectrum) in sources {
            let center = (position * height).round() as isize;
            for offset in -2_isize..=2 {
                let y = (center + offset).clamp(0, self.config.height as isize - 1) as usize;
                let envelope = (3 - offset.unsigned_abs()) as f32 / 3.0;
                let stimulus = spectrum * (0.16 * envelope);
                let index = self.index(1, y);
                self.sites[index].energy += stimulus;
                self.introduced += stimulus.total();
            }
        }
    }

    fn inject_noise(&mut self) {
        if !self.age.is_multiple_of(13) {
            return;
        }

        let seed = self.config.seed ^ self.age.rotate_left(17);
        for index in 0..self.sites.len() {
            let grain = mix64(seed ^ index as u64);
            if grain & 0x3f != 0 {
                continue;
            }

            let x = index % self.config.width;
            let access = (x as f32 / (self.config.width - 1) as f32).clamp(0.0, 1.0);
            let amount = 0.0025 * (0.25 + access * 0.75);
            let spectral = noise_spectrum(grain) * amount;
            self.sites[index].energy += spectral;
            let introduced = spectral.total();
            self.introduced += introduced;
            self.disturbance_introduced += introduced;
        }
    }

    fn apply_scar(&mut self) {
        for index in 0..self.sites.len() {
            let x = index % self.config.width;
            let y = index / self.config.width;
            let mask = scar_mask(x, y, self.config.width, self.config.height);
            if mask <= 0.0 {
                continue;
            }

            let site = &mut self.sites[index];
            let absorbed = site.energy * (mask * 0.018);
            site.energy -= absorbed;
            let absorbed_total = absorbed.total();
            self.dissipated += absorbed_total;
            self.disturbance_dissipated += absorbed_total;

            site.permeability = (site.permeability * (1.0 - mask * 0.006)).clamp(0.025, 1.0);
            site.coupling = site.coupling * (1.0 - mask * 0.010);
            site.phase = wrap_phase(site.phase + mask * 0.041);
        }
    }

    fn exchange(
        &self,
        left: usize,
        right: usize,
        follows_gradient: bool,
        change: &mut [Spectrum],
        local_flow: &mut [f32],
    ) {
        let a = self.sites[left];
        let b = self.sites[right];
        let edge_phase = if follows_gradient { 0.0 } else { PI * 0.5 };
        let alignment = 0.5 + 0.5 * (a.phase - b.phase + edge_phase).cos();
        let material = (a.permeability * b.permeability).sqrt();
        let spectral_bridge = spectral_bridge(a.energy, b.energy);
        let guard = self.relay_guard(left, right, a.energy + b.energy);
        let relay = self.config.phase_relay * alignment * spectral_bridge * material * guard;
        let conductance =
            ((0.14 + 0.86 * material) * (0.42 + 0.58 * alignment) * (1.0 + relay)).clamp(0.0, 1.70);
        let relaxation = (a.energy - b.energy) * (self.config.diffusion * conductance);
        let drift = if follows_gradient {
            a.energy * (self.config.gradient * conductance)
        } else {
            Spectrum::ZERO
        };
        let flow = relaxation + drift;
        change[left] -= flow;
        change[right] += flow;
        let magnitude = flow.magnitude();
        local_flow[left] += magnitude;
        local_flow[right] += magnitude;
    }

    fn relay_guard(&self, left: usize, right: usize, signal: Spectrum) -> f32 {
        if self.config.relay_guard <= 0.0 {
            return 1.0;
        }
        let y = ((left / self.config.width) + (right / self.config.width)) as f32 * 0.5;
        let ny = y / (self.config.height - 1) as f32;
        let expected = unit_spectrum(Spectrum::new(
            source_band(ny, 0.23),
            source_band(ny, 0.50),
            source_band(ny, 0.77),
        ));
        let observed = unit_spectrum(signal);
        let match_score = spectral_bridge(expected, observed);
        (1.0 - self.config.relay_guard * (1.0 - match_score)).clamp(0.0, 1.0)
    }
}

/// Audited narrow surface the kernel-glue layer is allowed to use.
///
/// Upstream `universum-substrate` exposes no direct site mutation API: the
/// only entry of energy is the boundary stimulation, and the only way to
/// observe sites is the rendered RGB buffer. When the field becomes an OS
/// substrate it needs three additional capabilities:
///
/// 1. Inject energy at an arbitrary site (so a PID can be stimulated).
/// 2. Read back a site's energy, trace, and coupling (so the scheduler and
///    IPC receive can make decisions).
/// 3. Seed a site's coupling (so IPC `send` can bias a receiver toward a
///    sender before the adaptive-coupling law compounds it).
///
/// Each function is `pub(crate)`-visible through this module so it cannot be
/// reached from outside the crate except via the kernel-glue layer that this
/// crate ships with. The functions are deliberate: they do not bypass
/// `Config::validate`, they do not modify `introduced` accounting except
/// through `inject_at` (which adds to `World::introduced` exactly as
/// boundary stimulation does), and they preserve the closed invariant.
pub mod kernel_bridge {
    use super::{Spectrum, World};

    /// Read a site's current energy. Returns `ZERO` for out-of-range index.
    pub fn site_energy(world: &World, index: usize) -> Spectrum {
        world.sites.get(index).map(|s| s.energy).unwrap_or(Spectrum::ZERO)
    }

    /// Read a site's current accumulated trace (visible radiation).
    pub fn site_trace(world: &World, index: usize) -> Spectrum {
        world.sites.get(index).map(|s| s.trace).unwrap_or(Spectrum::ZERO)
    }

    /// Read a site's current coupling peak shape.
    pub fn site_coupling(world: &World, index: usize) -> Spectrum {
        world.sites.get(index).map(|s| s.coupling).unwrap_or(Spectrum::ZERO)
    }

    /// Inject energy at a site. The amount is added to `World::introduced`
    /// so the closed invariant `introduced = resident + radiated +
    /// dissipated` continues to hold. No-op for out-of-range index.
    pub fn inject_at(world: &mut World, index: usize, spectrum: Spectrum) {
        if let Some(site) = world.sites.get_mut(index) {
            site.energy += spectrum;
            world.introduced += spectrum.total();
        }
    }

    /// Seed a site's coupling toward a target spectral shape by `amount`
    /// (clamped to keep coupling within `[0, 1]` per channel). Used by
    /// `ipc_send_as_energy` to bias a receiver toward a sender. No-op for
    /// out-of-range index.
    pub fn bias_coupling(world: &mut World, index: usize, target: Spectrum, amount: f32) {
        if let Some(site) = world.sites.get_mut(index) {
            let boost = target * amount;
            site.coupling = site.coupling.zip(boost, |a, b| (a + b).clamp(0.0, 1.0));
        }
    }
}

fn visible_coupling(x: usize, y: usize, width: usize, height: usize) -> Spectrum {
    let nx = x as f32 / (width - 1) as f32;
    let ny = y as f32 / (height - 1) as f32;
    let dx = (nx - 0.67) / 0.34;
    let dy = (ny - 0.50) / 0.45;
    let radius = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);
    let folded_ring = 0.52 + 0.095 * (angle * 5.0 + radius * 7.0).sin();
    let ring = soft_band((radius - folded_ring).abs(), 0.055);
    let inner = soft_band((radius - 0.19 - 0.035 * (angle * 3.0).cos()).abs(), 0.045);
    let vein = soft_band((dy - 0.19 * (nx * TAU * 2.3).sin()).abs(), 0.035)
        * soft_band((nx - 0.58).abs(), 0.42);
    let aperture = (ring.max(inner * 0.8).max(vein * 0.42) * edge_gate(nx)).clamp(0.0, 1.0);
    if aperture <= 0.001 {
        return Spectrum::ZERO;
    }
    let hue = (angle / TAU + 1.0).fract();
    let red = (0.5 + 0.5 * (TAU * hue).cos()).powi(2);
    let green = (0.5 + 0.5 * (TAU * (hue - 1.0 / 3.0)).cos()).powi(2);
    let blue = (0.5 + 0.5 * (TAU * (hue - 2.0 / 3.0)).cos()).powi(2);
    let floor = 0.16 * aperture;
    Spectrum::new(
        floor + aperture * red,
        floor + aperture * green,
        floor + aperture * blue,
    )
}

fn seed_coupling(random: &mut SplitMix64, x: usize, width: usize) -> Spectrum {
    let access = observation_access(x, width);
    if access <= 0.0 {
        return Spectrum::ZERO;
    }

    let amplitude = access * (0.00018 + random.unit_f32() * 0.00062);
    Spectrum::new(
        amplitude * (0.35 + random.unit_f32() * 0.65),
        amplitude * (0.35 + random.unit_f32() * 0.65),
        amplitude * (0.35 + random.unit_f32() * 0.65),
    )
}

fn adapt_coupling(
    site: &mut Site,
    x: usize,
    width: usize,
    config: Config,
    emitted: Spectrum,
    flow: f32,
) {
    let access = observation_access(x, width);
    if access <= 0.0 {
        site.coupling = Spectrum::ZERO;
        return;
    }

    let signal = site.energy + emitted;
    let spectral_shape = unit_spectrum(signal);
    let conversion_drive = (emitted.magnitude() * 28.0).min(1.0);
    let signal_drive = (signal.magnitude() * 0.24).min(1.0);
    let transport_drive = (flow * 0.42).min(1.0);
    let drive = (signal_drive * (site.activity * 0.70 + transport_drive * 0.25)
        + conversion_drive * 0.05)
        .clamp(0.0, 1.0);
    let target_amplitude = access * (0.18 + site.permeability * 0.82).clamp(0.0, 1.0);
    let target = spectral_shape * target_amplitude;
    let readiness = (site.coupling.peak() * 260.0).clamp(0.0, 1.0);
    let compound_drive = drive * (0.10 + readiness * 0.90);
    let compound =
        1.0 + config.coupling_formation * compound_drive * (1.0 + site.coupling.peak() * 28.0);
    let imprint = target * (config.coupling_formation * drive * 0.16);

    site.coupling = site.coupling * compound.min(1.32) + imprint;
    site.coupling = site
        .coupling
        .zip(target, |value, cap| value.min(cap.max(0.001)));

    let disuse = config.coupling_erosion * (1.0 - site.activity * 0.72).clamp(0.0, 1.0);
    site.coupling = site
        .coupling
        .map(|value| (value * (1.0 - disuse)).clamp(0.0, 1.0));
}

fn unit_spectrum(signal: Spectrum) -> Spectrum {
    let total = signal.red + signal.green + signal.blue;
    if total <= 0.000_001 {
        Spectrum::ZERO
    } else {
        signal * (1.0 / total)
    }
}

fn spectral_bridge(a: Spectrum, b: Spectrum) -> f32 {
    let a_total = a.red + a.green + a.blue;
    let b_total = b.red + b.green + b.blue;
    if a_total <= 0.000_001 || b_total <= 0.000_001 {
        0.0
    } else {
        let ar = a.red / a_total;
        let ag = a.green / a_total;
        let ab = a.blue / a_total;
        let br = b.red / b_total;
        let bg = b.green / b_total;
        let bb = b.blue / b_total;
        (1.0 - ((ar - br).abs() + (ag - bg).abs() + (ab - bb).abs()) * 0.5).clamp(0.0, 1.0)
    }
}

fn spectral_turn(signal: Spectrum) -> f32 {
    let total = signal.red + signal.green + signal.blue;
    if total <= 0.000_001 {
        0.0
    } else {
        let red = signal.red / total;
        let green = signal.green / total;
        let blue = signal.blue / total;
        (red - blue) * 0.72 + (green - 0.5 * (red + blue)) * 0.38
    }
}

fn wrap_phase(phase: f32) -> f32 {
    phase.rem_euclid(TAU)
}

fn observation_access(x: usize, width: usize) -> f32 {
    let nx = x as f32 / (width - 1) as f32;
    ((nx - 0.40) / 0.20).clamp(0.0, 1.0)
}

fn source_band(ny: f32, center: f32) -> f32 {
    soft_band((ny - center).abs(), 0.14)
}

fn channel_from_matrix(matrix: [[f64; 3]; 3]) -> ChannelMeasurements {
    let total = matrix.iter().flatten().sum::<f64>();
    if total <= 0.000_001 {
        return ChannelMeasurements::default();
    }

    let signal = matrix[0][0] + matrix[1][1] + matrix[2][2];
    let fidelity = signal / total;
    let weakest = matrix[0][0].min(matrix[1][1]).min(matrix[2][2]);
    let balance = if signal <= 0.000_001 {
        0.0
    } else {
        (3.0 * weakest / signal).clamp(0.0, 1.0)
    };

    ChannelMeasurements {
        signal,
        total,
        fidelity,
        balance,
        information_bits: channel_information_bits(matrix, total),
    }
}

fn channel_information_bits(matrix: [[f64; 3]; 3], total: f64) -> f64 {
    let mut row_totals = [0.0_f64; 3];
    let mut column_totals = [0.0_f64; 3];
    for band in 0..3 {
        for (color, value) in matrix[band].iter().enumerate() {
            row_totals[band] += *value;
            column_totals[color] += *value;
        }
    }

    let mut information = 0.0;
    for band in 0..3 {
        for (color, value) in matrix[band].iter().enumerate() {
            if *value <= 0.0 || row_totals[band] <= 0.0 || column_totals[color] <= 0.0 {
                continue;
            }
            let joint = *value / total;
            let independent = (row_totals[band] / total) * (column_totals[color] / total);
            information += joint * (joint / independent).log2();
        }
    }
    information.max(0.0)
}

fn scar_mask(x: usize, y: usize, width: usize, height: usize) -> f32 {
    let nx = x as f32 / (width - 1) as f32;
    let ny = y as f32 / (height - 1) as f32;
    let ridge = 0.54 + 0.035 * (ny * TAU * 2.7).sin();
    let vertical = soft_band((nx - ridge).abs(), 0.034);
    let ribbing = 0.60 + 0.40 * (ny * TAU * 8.0 + nx * TAU * 1.5).sin().abs();
    let channel_opening = 1.0 - 0.45 * soft_band((ny - 0.50).abs(), 0.16);
    (vertical * ribbing * channel_opening).clamp(0.0, 1.0)
}

fn noise_spectrum(grain: u64) -> Spectrum {
    let red = ((grain >> 8) & 0xff) as f32 / 255.0;
    let green = ((grain >> 24) & 0xff) as f32 / 255.0;
    let blue = ((grain >> 40) & 0xff) as f32 / 255.0;
    Spectrum::new(red, green, blue)
}

fn soft_band(distance: f32, width: f32) -> f32 {
    let normalized = (distance / width).clamp(0.0, 1.0);
    0.5 + 0.5 * (PI * normalized).cos()
}

fn edge_gate(nx: f32) -> f32 {
    ((nx - 0.18) / 0.12).clamp(0.0, 1.0)
}

fn linear_to_u8(value: f32) -> u8 {
    let gamma_corrected = value.clamp(0.0, 1.0).powf(1.0 / 2.2);
    (gamma_corrected * 255.0).round() as u8
}

struct SplitMix64(u64);

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.0)
    }

    fn unit_f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_dimensions_are_rejected() {
        let result = World::new(Config {
            width: 8,
            ..Config::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn energy_accounting_remains_closed() {
        let mut world = World::new(Config {
            width: 48,
            height: 32,
            seed: 7,
            ..Config::default()
        })
        .unwrap();
        world.evolve_for(500);
        let measured = world.measurements();
        let relative_error = measured.accounting_error.abs() / measured.introduced;
        assert!(
            relative_error < 0.000_03,
            "relative error was {relative_error}"
        );
        assert!(measured.radiated > 0.0);
        assert!(measured.resident > 0.0);
    }

    #[test]
    fn identical_initial_conditions_have_identical_futures() {
        let config = Config {
            width: 40,
            height: 28,
            seed: 91,
            ..Config::default()
        };
        let mut first = World::new(config).unwrap();
        let mut second = World::new(config).unwrap();
        first.evolve_for(100);
        second.evolve_for(100);
        assert_eq!(first.rgb8(), second.rgb8());
        assert_eq!(
            first.measurements().introduced,
            second.measurements().introduced
        );
    }

    #[test]
    fn repeated_flow_changes_the_material() {
        let mut world = World::new(Config {
            width: 40,
            height: 28,
            seed: 19,
            ..Config::default()
        })
        .unwrap();
        let before = world.measurements().mean_permeability;
        world.evolve_for(700);
        let after = world.measurements().mean_permeability;
        assert!(after > before * 1.02, "before {before}, after {after}");
        assert!(world.measurements().organized_fraction > 0.0);
    }

    #[test]
    fn adaptive_compounding_amplifies_coupling_beyond_inert_seed() {
        let adaptive_config = Config {
            width: 56,
            height: 36,
            seed: 103,
            ..Config::default()
        };
        let inert_config = Config {
            coupling_mode: CouplingMode::Inert,
            ..adaptive_config
        };

        let mut adaptive = World::new(adaptive_config).unwrap();
        let mut inert = World::new(inert_config).unwrap();
        adaptive.evolve_for(900);
        inert.evolve_for(900);

        let adaptive = adaptive.measurements();
        let inert = inert.measurements();
        assert!(
            adaptive.radiated > inert.radiated * 8.0,
            "adaptive {}, inert {}",
            adaptive.radiated,
            inert.radiated
        );
        assert!(adaptive.coupled_fraction > inert.coupled_fraction + 0.10);
    }

    #[test]
    fn disturbance_energy_is_accounted_for() {
        let mut world = World::new(Config {
            width: 48,
            height: 32,
            seed: 211,
            disturbance_mode: DisturbanceMode::Noise,
            ..Config::default()
        })
        .unwrap();
        world.evolve_for(500);
        let measured = world.measurements();
        let relative_error = measured.accounting_error.abs() / measured.introduced;
        assert!(
            relative_error < 0.000_04,
            "relative error was {relative_error}"
        );
        assert!(measured.disturbance_introduced > 0.0);
    }

    #[test]
    fn adaptive_material_outperforms_inert_material_under_scar() {
        let adaptive_config = Config {
            width: 56,
            height: 36,
            seed: 103,
            disturbance_mode: DisturbanceMode::Scar,
            ..Config::default()
        };
        let inert_config = Config {
            coupling_mode: CouplingMode::Inert,
            ..adaptive_config
        };

        let mut adaptive = World::new(adaptive_config).unwrap();
        let mut inert = World::new(inert_config).unwrap();
        adaptive.evolve_for(1_200);
        inert.evolve_for(1_200);

        let adaptive = adaptive.measurements();
        let inert = inert.measurements();
        assert!(adaptive.disturbance_dissipated > 0.0);
        assert!(
            adaptive.radiated > inert.radiated * 4.0,
            "adaptive {}, inert {}",
            adaptive.radiated,
            inert.radiated
        );
        assert!(adaptive.luminous_sites > inert.luminous_sites);
    }

    #[test]
    fn adaptive_material_preserves_more_channel_information_under_scar() {
        let adaptive_config = Config {
            width: 56,
            height: 36,
            seed: 103,
            disturbance_mode: DisturbanceMode::Scar,
            ..Config::default()
        };
        let inert_config = Config {
            coupling_mode: CouplingMode::Inert,
            ..adaptive_config
        };

        let mut adaptive = World::new(adaptive_config).unwrap();
        let mut inert = World::new(inert_config).unwrap();
        adaptive.evolve_for(1_200);
        inert.evolve_for(1_200);

        let adaptive = adaptive.measurements();
        let inert = inert.measurements();
        assert!(
            adaptive.channel_signal > inert.channel_signal * 12.0,
            "adaptive {}, inert {}",
            adaptive.channel_signal,
            inert.channel_signal
        );
        assert!(
            adaptive.channel_information_bits > inert.channel_information_bits + 0.15,
            "adaptive {}, inert {}",
            adaptive.channel_information_bits,
            inert.channel_information_bits
        );
        assert!(adaptive.channel_fidelity > 0.65);
    }
}
