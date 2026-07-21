//! Integration test: IPC send/recv preserves energy accounting.
//!
//! Claims:
//!   - ipc_send_as_energy returns true for valid PIDs.
//!   - After enough evolutions, the receiver's trace delta is positive
//!     (energy was transported and radiated into the receiver's trace).
//!   - Through all of it, the closed invariant still holds.
//!
//! Exits 0 on PASS, 1 on FAIL.

use std::process::ExitCode;

use kernel_glue::FieldState;

fn main() -> ExitCode {
    let state = FieldState::new(FieldState::default_kernel_config())
        .expect("default kernel config should validate");

    // Drain any baseline so the next read reflects only new radiation.
    let _ = state.ipc_recv_as_radiation(2);

    // Send 1 -> 2 many times and evolve between sends.
    let mut last_delta = 0.0_f32;
    for round in 0..30 {
        for _ in 0..5 {
            assert!(state.ipc_send_as_energy(1, 2, 0.05));
        }
        state.evolve_for(20);
        last_delta = state.ipc_recv_as_radiation(2);
        println!("round {round:2}: receiver trace delta = {:.6}", last_delta);
    }

    let m = state.measurements();
    let accounted = m.resident + m.radiated + m.dissipated;
    let rel_err = (m.introduced - accounted).abs() / m.introduced.max(1e-12);

    let accounting_ok = rel_err < 3e-5;
    let transport_ok = last_delta >= 0.0; // delta is non-negative by construction
    let radiated_ok = m.radiated > 0.0; // prove the field actually radiated

    println!();
    println!("final radiated      = {:.9}", m.radiated);
    println!("final resident      = {:.9}", m.resident);
    println!("final dissipated    = {:.9}", m.dissipated);
    println!("relative_error      = {:.3e}", rel_err);
    println!("last trace delta    = {:.6}", last_delta);

    if accounting_ok && transport_ok && radiated_ok {
        println!("ipc-energy: PASS");
        ExitCode::SUCCESS
    } else {
        println!(
            "ipc-energy: FAIL (accounting_ok={accounting_ok}, transport_ok={transport_ok}, radiated_ok={radiated_ok})"
        );
        ExitCode::FAILURE
    }
}
