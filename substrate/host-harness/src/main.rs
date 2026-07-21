//! Host-side falsify/sweep harness.
//!
//! Runs the universum primitive-stack falsification against field-core. The
//! goal is to demonstrate that the no_std port reproduces the upstream
//! adaptive radiation gain (39.95x in README.md:120). The harness is std-only
//! so it can use std::time::Instant and write artifacts to disk; the physics
//! under test runs entirely through field-core, which when built under std
//! uses the same libm-backed math it uses in the kernel.
//!
//! Usage:
//!     cargo run -p host-harness --release -- falsify [--width N] [--height N]
//!                                                       [--moments N] [--seed N]
//!                                                       [--output PATH]
//!     cargo run -p host-harness --release -- check
//!
//! The `check` subcommand runs a single small deterministic configuration and
//! prints PASS/FAIL only.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use field_core::{Config, CouplingMode, DisturbanceMode, World};

const STANDARD_ORDER: &str =
    "boundary_stimulation -> local_energy_flow -> permeability_formation -> \
     erosion -> spectral_coupling -> radiation -> dissipation";

#[derive(Clone, Copy, Debug)]
struct PrimitiveStack {
    name: &'static str,
    coupling_mode: CouplingMode,
    disturbance_mode: DisturbanceMode,
    width: usize,
    height: usize,
    seed: u64,
    moments: u64,
}

impl PrimitiveStack {
    fn config(self) -> Config {
        Config {
            width: self.width,
            height: self.height,
            seed: self.seed,
            coupling_mode: self.coupling_mode,
            disturbance_mode: self.disturbance_mode,
            ..Config::default()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct StackOutcome {
    stack: PrimitiveStack,
    radiated: f64,
    channel_information_bits: f64,
    disturbance_introduced: f64,
    luminous_sites: usize,
    relative_accounting_error: f64,
    visible_hash: u64,
    repeated_visible_hash: u64,
    deterministic: bool,
    elapsed_seconds: f64,
}

impl StackOutcome {
    fn evolve(stack: PrimitiveStack) -> Result<Self, String> {
        let started = Instant::now();
        let mut world = World::new(stack.config())?;
        world.evolve_for(stack.moments);
        let elapsed_seconds = started.elapsed().as_secs_f64();
        let measurements = world.measurements();
        let visible_hash = world.visible_hash64();

        let mut repeated = World::new(stack.config())?;
        repeated.evolve_for(stack.moments);
        let repeated_visible_hash = repeated.visible_hash64();
        let repeated_measurements = repeated.measurements();
        let deterministic = visible_hash == repeated_visible_hash
            && same_measurements(measurements, repeated_measurements);
        let relative_accounting_error = if measurements.introduced.abs() <= f64::EPSILON {
            measurements.accounting_error.abs()
        } else {
            measurements.accounting_error.abs() / measurements.introduced.abs()
        };

        Ok(Self {
            stack,
            radiated: measurements.radiated,
            channel_information_bits: measurements.channel_information_bits,
            disturbance_introduced: measurements.disturbance_introduced,
            luminous_sites: measurements.luminous_sites,
            relative_accounting_error,
            visible_hash,
            repeated_visible_hash,
            deterministic,
            elapsed_seconds,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct FalsificationThresholds {
    max_relative_accounting_error: f64,
    min_adaptive_radiation_gain: f64,
    min_adaptive_channel_info_gain: f64,
    min_scar_radiation_gain: f64,
    min_scar_channel_info_gain: f64,
    min_noise_luminous_sites: usize,
}

impl Default for FalsificationThresholds {
    fn default() -> Self {
        Self {
            max_relative_accounting_error: 0.000_05,
            min_adaptive_radiation_gain: 8.0,
            min_adaptive_channel_info_gain: 0.08,
            min_scar_radiation_gain: 4.0,
            min_scar_channel_info_gain: 0.15,
            min_noise_luminous_sites: 1,
        }
    }
}

fn run_standard_falsification(
    width: usize,
    height: usize,
    seed: u64,
    moments: u64,
    thresholds: FalsificationThresholds,
) -> Result<(String, bool, f64), String> {
    let standard = |name, coupling, disturbance| PrimitiveStack {
        name,
        coupling_mode: coupling,
        disturbance_mode: disturbance,
        width,
        height,
        seed,
        moments,
    };

    let baseline = StackOutcome::evolve(standard(
        "adaptive-none",
        CouplingMode::Adaptive,
        DisturbanceMode::None,
    ))?;
    let inert = StackOutcome::evolve(standard(
        "inert-none",
        CouplingMode::Inert,
        DisturbanceMode::None,
    ))?;
    let fixed = StackOutcome::evolve(standard(
        "fixed-none",
        CouplingMode::Fixed,
        DisturbanceMode::None,
    ))?;
    let scar_adaptive = StackOutcome::evolve(standard(
        "adaptive-scar",
        CouplingMode::Adaptive,
        DisturbanceMode::Scar,
    ))?;
    let scar_inert = StackOutcome::evolve(standard(
        "inert-scar",
        CouplingMode::Inert,
        DisturbanceMode::Scar,
    ))?;
    let noise_adaptive = StackOutcome::evolve(standard(
        "adaptive-noise",
        CouplingMode::Adaptive,
        DisturbanceMode::Noise,
    ))?;

    let outcomes = [
        baseline, inert, fixed, scar_adaptive, scar_inert, noise_adaptive,
    ];

    let mut failures = Vec::new();
    for outcome in outcomes {
        if !outcome.deterministic {
            failures.push(format!(
                "{}: identical stack run produced different visible or measured state",
                outcome.stack.name
            ));
        }
        if outcome.relative_accounting_error > thresholds.max_relative_accounting_error {
            failures.push(format!(
                "{}: energy accounting error {} exceeded {}",
                outcome.stack.name,
                outcome.relative_accounting_error,
                thresholds.max_relative_accounting_error
            ));
        }
    }

    let adaptive_radiation_gain = ratio(baseline.radiated, inert.radiated);
    let adaptive_channel_information_gain =
        baseline.channel_information_bits - inert.channel_information_bits;
    let scar_radiation_gain = ratio(scar_adaptive.radiated, scar_inert.radiated);
    let scar_channel_information_gain =
        scar_adaptive.channel_information_bits - scar_inert.channel_information_bits;

    if adaptive_radiation_gain <= thresholds.min_adaptive_radiation_gain {
        failures.push(format!(
            "adaptive stack did not radiate enough beyond inert (gain={})",
            adaptive_radiation_gain
        ));
    }
    if adaptive_channel_information_gain <= thresholds.min_adaptive_channel_info_gain {
        failures.push(format!(
            "adaptive stack did not add enough channel info (gain={})",
            adaptive_channel_information_gain
        ));
    }
    if scar_radiation_gain <= thresholds.min_scar_radiation_gain {
        failures.push(format!(
            "adaptive scar stack did not preserve radiation (gain={})",
            scar_radiation_gain
        ));
    }
    if scar_channel_information_gain <= thresholds.min_scar_channel_info_gain {
        failures.push(format!(
            "adaptive scar stack did not preserve channel info (gain={})",
            scar_channel_information_gain
        ));
    }
    if noise_adaptive.disturbance_introduced <= 0.0 {
        failures.push("noise stack did not introduce tracked disturbance energy".into());
    }
    if noise_adaptive.luminous_sites < thresholds.min_noise_luminous_sites {
        failures.push("noise stack produced no visible state".into());
    }

    let passed = failures.is_empty();

    let mut report = String::new();
    report.push_str("Universum primitive-stack falsification report (host-harness)\n");
    report.push_str("schema=1\n");
    report.push_str("operator_disposition=none\n");
    report.push_str("stacking_order_critical=true\n");
    report.push_str("all_primitives_reported_before_disposition=true\n");
    report.push_str(&format!(
        "max_relative_accounting_error={:.9}\n",
        thresholds.max_relative_accounting_error
    ));
    report.push('\n');
    for outcome in outcomes {
        report.push_str(&format_outcome(outcome));
    }
    report.push_str(&format!(
        "adaptive_radiation_gain={:.9}\n",
        adaptive_radiation_gain
    ));
    report.push_str(&format!(
        "adaptive_channel_information_gain={:.9}\n",
        adaptive_channel_information_gain
    ));
    report.push_str(&format!("scar_radiation_gain={:.9}\n", scar_radiation_gain));
    report.push_str(&format!(
        "scar_channel_information_gain={:.9}\n",
        scar_channel_information_gain
    ));
    if passed {
        report.push_str("falsification_gate=PASS\n");
    } else {
        report.push_str("falsification_gate=FAIL\n");
        for failure in &failures {
            report.push_str(&format!("failure={failure}\n"));
        }
    }

    Ok((report, passed, adaptive_radiation_gain))
}

fn format_outcome(outcome: StackOutcome) -> String {
    let mut s = String::new();
    s.push_str(&format!("stack={}\n", outcome.stack.name));
    s.push_str(&format!("  order={STANDARD_ORDER}\n"));
    s.push_str(&format!(
        "  coupling={}\n",
        outcome.stack.coupling_mode.as_str()
    ));
    s.push_str(&format!(
        "  disturbance={}\n",
        outcome.stack.disturbance_mode.as_str()
    ));
    s.push_str(&format!("  width={}\n", outcome.stack.width));
    s.push_str(&format!("  height={}\n", outcome.stack.height));
    s.push_str(&format!("  seed={}\n", outcome.stack.seed));
    s.push_str(&format!("  moments={}\n", outcome.stack.moments));
    s.push_str(&format!("  elapsed_seconds={:.6}\n", outcome.elapsed_seconds));
    s.push_str(&format!("  radiated={:.9}\n", outcome.radiated));
    s.push_str(&format!(
        "  channel_information_bits={:.9}\n",
        outcome.channel_information_bits
    ));
    s.push_str(&format!(
        "  disturbance_introduced={:.9}\n",
        outcome.disturbance_introduced
    ));
    s.push_str(&format!(
        "  luminous_sites={}\n",
        outcome.luminous_sites
    ));
    s.push_str(&format!(
        "  relative_accounting_error={:.12}\n",
        outcome.relative_accounting_error
    ));
    s.push_str(&format!("  visible_hash=0x{:016x}\n", outcome.visible_hash));
    s.push_str(&format!(
        "  repeated_visible_hash=0x{:016x}\n",
        outcome.repeated_visible_hash
    ));
    s.push_str(&format!("  deterministic={}\n", outcome.deterministic));
    s
}

fn same_measurements(
    a: field_core::Measurements,
    b: field_core::Measurements,
) -> bool {
    a.age == b.age
        && a.introduced.to_bits() == b.introduced.to_bits()
        && a.resident.to_bits() == b.resident.to_bits()
        && a.radiated.to_bits() == b.radiated.to_bits()
        && a.dissipated.to_bits() == b.dissipated.to_bits()
        && a.accounting_error.to_bits() == b.accounting_error.to_bits()
        && a.channel_information_bits.to_bits() == b.channel_information_bits.to_bits()
}

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() <= f64::EPSILON {
        f64::INFINITY
    } else {
        numerator / denominator
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

struct Options {
    width: usize,
    height: usize,
    moments: u64,
    seed: u64,
    output: PathBuf,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 56,
            height: 36,
            moments: 1_200,
            seed: 103,
            output: PathBuf::from("artifacts/host-falsify.txt"),
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("falsify") => match run_falsify(&args[1..]) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(message) => {
                eprintln!("host-harness: {message}");
                ExitCode::FAILURE
            }
        },
        Some("check") => match run_check() {
            Ok(passed) => {
                if passed {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(message) => {
                eprintln!("host-harness: {message}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("host-harness: unknown subcommand '{other}'");
            eprintln!("usage: host-harness <falsify|check> [options]");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: host-harness <falsify|check> [options]");
            ExitCode::FAILURE
        }
    }
}

fn run_falsify(args: &[String]) -> Result<bool, String> {
    let mut options = Options::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--width" => {
                i += 1;
                options.width = parse_usize(arg, &args[i])?;
            }
            "--height" => {
                i += 1;
                options.height = parse_usize(arg, &args[i])?;
            }
            "--moments" => {
                i += 1;
                options.moments = parse_u64(arg, &args[i])?;
            }
            "--seed" => {
                i += 1;
                options.seed = parse_u64(arg, &args[i])?;
            }
            "--output" => {
                i += 1;
                options.output = PathBuf::from(&args[i]);
            }
            "-h" | "--help" => {
                println!("host-harness falsify [--width N] [--height N] [--moments N] [--seed N] [--output PATH]");
                return Ok(true);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    if options.moments == 0 {
        return Err("--moments must be greater than zero".into());
    }

    let (report, passed, adaptive_radiation_gain) = run_standard_falsification(
        options.width,
        options.height,
        options.seed,
        options.moments,
        FalsificationThresholds::default(),
    )?;

    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("could not create report directory: {e}"))?;
    }
    fs::write(&options.output, &report)
        .map_err(|e| format!("could not write report: {e}"))?;

    println!("{report}");
    println!("artifact: {}", options.output.display());
    println!(
        "adaptive_radiation_gain is {:.3}x (target >= 39x; upstream reports 39.95x)",
        adaptive_radiation_gain
    );
    Ok(passed)
}

fn run_check() -> Result<bool, String> {
    // Small deterministic config so `cargo run -- check` is fast in CI.
    let (_report, passed, adaptive_radiation_gain) = run_standard_falsification(
        40,
        28,
        103,
        650,
        FalsificationThresholds::default(),
    )?;
    println!(
        "check: adaptive_radiation_gain={:.3}, gate={}",
        adaptive_radiation_gain,
        if passed { "PASS" } else { "FAIL" }
    );
    Ok(passed)
}

fn parse_usize(name: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value for {name}: {value}"))
}

fn parse_u64(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value for {name}: {value}"))
}
