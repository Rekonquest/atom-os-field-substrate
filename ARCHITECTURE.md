# Atom OS — Field Substrate Bridge Architecture

This document describes how the radiation/dissipation field from
`universum-substrate` becomes the computational substrate of the Atom OS
kernel. It is the design counterpart to `STUDY.md` (which establishes ground
truth) and `APPLY.md` (which describes how to apply the kernel patch).

## Goal

Make the field the **single computational substrate** of the kernel, serving
three roles simultaneously:

1. **Kernel service** — the field evolves one moment per timer tick and is
   observable through new syscalls.
2. **IPC substrate** — `SYS_IPC_SEND`/`SYS_IPC_RECV` re-implemented as
   spectral-energy propagation followed by radiation absorption.
3. **Scheduler** — round-robin replaced by field-driven selection; the site
   with the highest `energy × coupling` gets the next time slice.

## Workspace layout

All new code lives inside `C:\Projects\ATOM OS\substrate`:

```
substrate\
├── Cargo.toml                # workspace manifest
├── field-core\               # no_std + alloc port of universum field.rs
│   ├── Cargo.toml            # no_std, default-features = false, dep: libm
│   └── src\lib.rs            # field.rs adapted
├── field-std\                # std-only facade re-exporting field-core for host tests
│   └── src\lib.rs
├── kernel-glue\              # no_std policy layer: field ↔ kernel mapping
│   └── src\lib.rs
├── host-harness\             # std binary: runs falsify/sweep via field-core
│   └── src\main.rs
└── integration-tests\        # one test crate per claim
    ├── closed-energy\
    ├── ipc-energy\
    └── scheduler-determinism\
```

Plus, at the workspace root:

- `patches/0001-atom-os-field-substrate.patch` — generated unified diff against
  `atom-os-kernel` at commit `3fe46c8`.
- `APPLY.md` — patch prerequisites, application, rollback, smoke test.

## Field-state to OS-state mapping

The bridge is conceptually a re-interpretation of the field's variables. No
physics constants change. Each PID maps to a fixed site in the field; the
16-task scheduler maps to a 4x4 region.

| Field state | OS meaning |
|---|---|
| `site.energy` (RGB triple) | process ready-state / attention. High energy = schedulable. |
| `site.coupling` | IPC affinity — energy flows where coupling is high. |
| `site.permeability` | wakefulness / memory retention (decays under disuse). |
| `site.trace` | output emitted to observer (logs, syscall results). |
| `radiation` law | a process emitting visible output. |
| `dissipation` law | idle decay — a quiet process forgets and unwakes. |
| `exchange` law (gradient + relaxation) | priority propagation + IPC message flow. |
| `phase relay` | long-horizon cooperative scheduling of coupled processes. |

## Why this is sound

The field's defining property is the **closed invariant**
`introduced = resident + radiated + dissipated`, enforced to relative error
< 3e-5 over 500 moments (`field.rs:933-950`). When the field becomes the
substrate for IPC and scheduling, the same invariant becomes the OS-wide
accounting guarantee:

- **Energy injected by stimulation or IPC send** = `introduced`.
- **Energy currently held by processes** = `resident`.
- **Energy emitted as visible output** = `radiated`.
- **Energy lost to idle decay** = `dissipated`.

No work is lost, no priority invented, no message unaccounted. This is the
structural reason the field is a fit for an OS substrate, not just a sim.

## Field-core: the no_std port

The port preserves every numeric constant and every law exactly. The only
changes from the upstream `field.rs` are:

- `#![no_std]` + `extern crate alloc;`.
- `std::f32::consts::{PI, TAU}` → crate-level `const PI: f32 = ...; const TAU: f32 = ...;`.
- `Vec<T>` → `alloc::vec::Vec<T>`.
- `std::ops::*` → `core::ops::*`.
- `f32::sin/cos/ln_1p/powf/atan2/log2` → `libm` FFI (`sin_f32`, `cos_f32`,
  `log1pf`, `powf`, `atan2f`, `log2f`). All other operations (`abs`, `min`,
  `max`, `clamp`, `sqrt`, `rem_euclid`, `fract`) are already in `core`.

The five original universum tests are ported unchanged into `field-core`'s
`#[cfg(test)]` module. They run on the host target via the `field-std` facade.
Pass criterion: identical to upstream (relative accounting error < 3e-5,
identical-futures equality, adaptive coupling amplification, etc.).

If `libm` diverges from std math enough to break the closed-invariant test,
the fallback is a hand-rolled degree-7 Taylor `sin_f32`/`cos_f32`. The
deviation is documented in `NOTES.md`; test tolerance is widened only if
absolutely needed and the widening is called out as a regression.

## Kernel-glue: the policy layer

`kernel-glue` is the only place OS semantics are introduced. It wraps
`field-core::World` in a `Spinlock<World>` and exposes:

- `FieldState::new(config) -> FieldState` — owns the world and a per-PID
  `last_trace` snapshot.
- `evolve_once(&self)` — acquires the lock and advances one moment.
- `stimulate(&self, pid, spectrum)` — injects energy at the site mapped to
  `pid`.
- `ipc_send_as_energy(&self, src, dst, magnitude)` — injects energy at `src`
  with coupling biased toward `dst`; the existing `exchange` law propagates
  it. The closed invariant still holds because the injection counts as
  `introduced`.
- `ipc_recv_as_radiation(&self, pid) -> f32` — returns the delta in
  `site.trace.peak()` since the last call for this PID.
- `scheduler_pick(&self) -> usize` — returns the PID whose site maximises
  `energy.peak() * (0.3 + coupling.peak())`.
- `measurements(&self) -> Measurements` — passes through `World::measurements`.

`FieldState` is exposed as a kernel static by `kernel-kit/src/scheduler_glue.rs`
in the patch.

## IPC re-implementation

`SYS_IPC_SEND(15)` and `SYS_IPC_RECV(16)` keep their numbers and their
visible contract (`send(target_pid, ptr)`; `recv() -> vaddr`). Only the
internal implementation changes:

- `SEND` becomes `ipc_send_as_energy(current_pid, target_pid, message_size)`.
  The bytes are not copied; the *energy equivalent* of the message is injected
  and propagates to the target site over the next evolutions.
- `RECV` becomes `ipc_recv_as_radiation(current_pid)`. If the accumulated
  trace delta exceeds a threshold, the kernel maps a page at `0x300000`
  containing a synthetic representation of the absorbed message and returns
  its address; otherwise returns 0 (nothing ready).

The flat `IPC_MAILBOXES` array is removed from the patch's `syscall.rs`. The
syscall ABI numbers are unchanged so existing payloads (shell.elf, daemon.elf)
still link without recompilation.

## Scheduler re-implementation + GAP-5 fix

The scheduler rewrite is gated behind a cargo feature `field-scheduler` so
round-robin remains the fallback path until smoke-tested in QEMU. When the
feature is on:

- `Scheduler::switch_context` consults `kernel_glue::scheduler_pick()` to
  choose the next task rather than scanning for the next Ready slot.
- **GAP-5 fix**: the current implementation only swaps `rsp`. The patch
  changes the asm wrapper and the save path to save and restore the full trap
  frame (RIP, RSP, RFLAGS, RAX..R15). This eliminates the RIP-corruption bug
  that today causes the benchmark payload to print "1111".

The GAP-5 fix is described in detail in `APPLY.md` with a before/after of the
asm wrapper.

## Patch generation and scope

The patch is generated against `atom-os-kernel` at commit `3fe46c8` by
applying the changes by hand on a fresh clone, running `git diff`, and saving
the result as `patches/0001-atom-os-field-substrate.patch`. Patch scope:

- `kernel-kit/Cargo.toml` — add path deps for `field-core` and `kernel-glue`.
- `kernel-kit/src/lib.rs` — re-export the two new crates.
- `kernel-kit/src/scheduler_glue.rs` (new) — expose `FieldState` static.
- `kernel-orchestrator/src/scheduler.rs` — rewrite `switch_context`,
  feature-gated; fix GAP-5 unconditionally.
- `kernel-orchestrator/src/syscall.rs` — rewire IPC syscalls; add
  `SYS_FIELD_STIMULATE(17)`, `SYS_FIELD_EVOLVE(18)`, `SYS_FIELD_OBSERVE(19)`,
  `SYS_FIELD_MEASUREMENTS(20)`.
- `x86_64-kernel/src/main.rs` — call `kernel_glue::evolve_once()` in the timer
  IRQ handler once every N ticks (configurable divisor, default 10).

The patch is **not applied** by this project. Applying it is a separate
operator action that requires explicit approval, because it modifies a
project outside `C:\Projects\ATOM OS`.

## Verification strategy

Each milestone has a hard gate. See `NOTES.md` for the running log of
commands and their results. Summary:

| Milestone | Gate |
|---|---|
| M1 field-core | `cargo test -p field-core` passes all 5 original universum tests |
| M2 host-harness | `cargo run -p host-harness --release -- falsify` reproduces ≥39× adaptive radiation gain |
| M3 kernel-glue | `cargo test -p kernel-glue` passes |
| M4 integration tests | closed-energy, ipc-energy, scheduler-determinism all pass |
| M5 patch | `git apply --check patches/0001-...patch` succeeds on a fresh clone of `atom-os-kernel@3fe46c8` |
| M6 docs | APPLY.md + NOTES.md complete and consistent with verification output |

## Risks (carried over from the plan)

| Risk | Mitigation |
|---|---|
| libm floats diverge from std enough to break the 3e-5 invariant | Fall back to hand-rolled Taylor `sin_f32`/`cos_f32`; document deviation |
| Field cost per tick too high for a 100 Hz timer | `evolve_once()` runs only every N ticks (default N=10) |
| GAP-5 scheduler fix destabilizes boot | `field-scheduler` cargo feature, default off; round-robin remains fallback |
| Patch doesn't apply because upstream moved | Pin to commit `3fe46c8`; `APPLY.md` names the exact base |
| Determinism broken by kernel allocator | Field depends only on `Config.seed`, never on allocator output |
| Scheduler change breaks existing payloads | IPC syscall numbers preserved; only internals change |
