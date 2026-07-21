//! Integration test: same seed -> same scheduler selection sequence.
//!
//! Claims:
//!   - Two FieldStates built from the same config, driven by the same
//!     sequence of (stimulate, evolve) calls, return the same
//!     scheduler_pick() at every step.
//!   - The pick changes over time as energy propagates and dissipates
//!     (i.e. the scheduler is not stuck).
//!
//! Exits 0 on PASS, 1 on FAIL.

use std::process::ExitCode;

use field_core::Spectrum;
use kernel_glue::FieldState;

fn main() -> ExitCode {
    let a = FieldState::new(FieldState::default_kernel_config())
        .expect("default kernel config should validate");
    let b = FieldState::new(FieldState::default_kernel_config())
        .expect("default kernel config should validate");

    let mask: u16 = 0b0000_0000_1111_1111; // PIDs 0..8 ready
    let mut a_picks = Vec::new();
    let mut b_picks = Vec::new();
    let mut changes = 0;
    let mut prev: Option<usize> = None;

    for step in 0..40 {
        // Identical stimulus schedule on both fields.
        let stimulus_pid = (step % 6) as usize;
        assert!(a.stimulate(stimulus_pid, Spectrum::new(0.04, 0.02, 0.02)));
        assert!(b.stimulate(stimulus_pid, Spectrum::new(0.04, 0.02, 0.02)));
        a.evolve_for(10);
        b.evolve_for(10);
        let pa = a.scheduler_pick(mask).expect("non-empty mask");
        let pb = b.scheduler_pick(mask).expect("non-empty mask");
        if Some(pa) != prev {
            changes += 1;
            prev = Some(pa);
        }
        a_picks.push(pa);
        b_picks.push(pb);
    }

    println!("field A picks: {:?}", a_picks);
    println!("field B picks: {:?}", b_picks);
    println!("distinct transitions: {changes}");

    let deterministic = a_picks == b_picks;
    let dynamic = changes >= 2;

    if deterministic && dynamic {
        println!("scheduler-determinism: PASS");
        ExitCode::SUCCESS
    } else {
        println!(
            "scheduler-determinism: FAIL (deterministic={deterministic}, dynamic={dynamic})"
        );
        ExitCode::FAILURE
    }
}
