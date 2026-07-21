//! Integration test: closed invariant holds through kernel-glue.
//!
//! Claims: through the kernel-glue surface (stimulate, ipc_send_as_energy,
//! evolve_for), the field still satisfies
//!     introduced = resident + radiated + dissipated
//! to relative error < 3e-5 (the original field-core tolerance).
//!
//! Exits 0 on PASS, 1 on FAIL.

use std::process::ExitCode;

use field_core::Spectrum;
use kernel_glue::FieldState;

fn main() -> ExitCode {
    let state = FieldState::new(FieldState::default_kernel_config())
        .expect("default kernel config should validate");

    // Stimulate a few PIDs as if the kernel were scheduling them.
    for pid in 0..6 {
        assert!(state.stimulate(pid, Spectrum::new(0.05, 0.05, 0.05)));
    }
    state.evolve_for(500);

    // Add some IPC traffic on top.
    for _ in 0..10 {
        assert!(state.ipc_send_as_energy(1, 9, 0.04));
        assert!(state.ipc_send_as_energy(4, 12, 0.06));
    }
    state.evolve_for(500);

    let m = state.measurements();
    let accounted = m.resident + m.radiated + m.dissipated;
    let rel_err = (m.introduced - accounted).abs() / m.introduced.max(1e-12);

    println!("introduced      = {:.9}", m.introduced);
    println!("resident        = {:.9}", m.resident);
    println!("radiated        = {:.9}", m.radiated);
    println!("dissipated      = {:.9}", m.dissipated);
    println!("accounted       = {:.9}", accounted);
    println!("relative_error  = {:.3e}", rel_err);

    if rel_err < 3e-5 {
        println!("closed-energy: PASS");
        ExitCode::SUCCESS
    } else {
        println!("closed-energy: FAIL (rel_err={rel_err} >= 3e-5)");
        ExitCode::FAILURE
    }
}
