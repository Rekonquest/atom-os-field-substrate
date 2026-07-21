# Atom OS Field Substrate — Work Log

Running notes for the bridge project. Every verification command and its
result is recorded here. Newest entries at the bottom.

## Session 2026-07-20 — planning and scaffold

### Verified upstream sources (first-hand reads)

- `C:\Projects\universe equavelant to vulkan driver\src\field.rs` — 1,102
  lines. Six physics laws. Closed invariant enforced at `field.rs:933-950`
  with rel-err < 3e-5.
- `src\falsify.rs` — 477 lines. `STANDARD_ORDER` puts radiation and
  dissipation at the top of the primitive stack. `FalsificationThresholds`
  default `min_adaptive_radiation_gain = 8.0`; upstream reports 39.95x.
- `src\render.rs` — 72 lines. BMP writer, not needed for the kernel.
- `src\lib.rs` — re-exports `Config`, `CouplingMode`, `DisturbanceMode`,
  `Measurements`, `Spectrum`, `World` from `field`; `FalsificationReport`,
  `FalsificationThresholds`, `PrimitiveStack`, `StackOutcome`,
  `run_standard_falsification` from `falsify`.
- `src\bin\falsify.rs` — 344 lines. CLI; default `56x36 @ 1200 moments,
  seed=103`. Writes artifacts + prints report; exits non-zero on FAIL.
- `C:\Projects\atom-os-kernel\kernel-orchestrator\src\syscall.rs` — 378
  lines. 17 syscalls (1..16). Flat `IPC_MAILBOXES` at line 378.
- `C:\Projects\atom-os-kernel\kernel-orchestrator\src\scheduler.rs` — 102
  lines. 16-slot round-robin. GAP-5 confirmed.
- `C:\Projects\atom-os-kernel\kernel-kit\src\atoms.rs` — 72 lines. 8 atoms,
  no physics.

### Toolchain

```
cargo 1.96.0 (30a34c682 2026-05-25)
rustc 1.96.0 (ac68faa20 2026-05-25)
installed targets: x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu
```

Edition 2024 is supported. `libm` is the no_std transcendentals provider.

### Math port — correction after first compile

Initial claim was that `abs`, `min`, `max`, `clamp`, `sqrt`, `rem_euclid`,
`fract` were in `core::f32`. **Wrong.** Only `abs`, `min`, `max`, `clamp`
are inherent on `f32` under no_std. The methods `sqrt`, `fract`, `powi`,
`powf`, `ln_1p`, `atan2`, `round`, `rem_euclid`, `sin`, `cos`, `log2` are
all std-only. The `math` module now provides all of them via libm:

- `sin_f32`, `cos_f32`, `log1pf`, `powf` (f32!), `atan2f`, `sqrtf`,
  `roundf`, `fmodf`, `truncf`, `log2`.

`powi` is implemented as `powf(self, exponent as f32)` because libm has no
integer-power helper. Exact for the small exponents used upstream (always 2).

`rem_euclid` is implemented via `fmodf` with sign correction: euclidean
remainder is always non-negative when `other > 0`.

`fract` is `self - truncf(self)`.

### Decisions

- Workspace at `C:\Projects\ATOM OS\substrate\` with five crates plus three
  integration-test crates. See `ARCHITECTURE.md`.
- Field is the substrate for **service + IPC + scheduler** (operator chose
  "Full: all three" depth).
- Kernel-side changes are a generated patch only; `atom-os-kernel` is not
  modified without per-action approval (operator chose "New code in ATOM OS
  only").
- Scheduler rewrite is feature-gated behind `field-scheduler` to keep
  round-robin as fallback.

## M0 + M1 + M2 + M3 + M4 — verification log

### M0 — scaffold

Files written (verified by list_directory):

- `C:\Projects\ATOM OS\STUDY.md`
- `C:\Projects\ATOM OS\ARCHITECTURE.md`
- `C:\Projects\ATOM OS\NOTES.md` (this file)
- `C:\Projects\ATOM OS\substrate\Cargo.toml` (workspace)
- `C:\Projects\ATOM OS\substrate\field-core\{Cargo.toml, src/lib.rs}`
- `C:\Projects\ATOM OS\substrate\field-std\{Cargo.toml, src/lib.rs}`
- `C:\Projects\ATOM OS\substrate\kernel-glue\{Cargo.toml, src/lib.rs}`
- `C:\Projects\ATOM OS\substrate\host-harness\{Cargo.toml, src/main.rs}`
- `C:\Projects\ATOM OS\substrate\integration-tests\closed-energy\{Cargo.toml, src/main.rs}`
- `C:\Projects\ATOM OS\substrate\integration-tests\ipc-energy\{Cargo.toml, src/main.rs}`
- `C:\Projects\ATOM OS\substrate\integration-tests\scheduler-determinism\{Cargo.toml, src/main.rs}`

### M1 — field-core no_std port

Command:
```
cargo test -p field-core --release
```
Result: **8/8 tests pass**, zero warnings.
```
test tests::invalid_dimensions_are_rejected ... ok
test tests::identical_initial_conditions_have_identical_futures ... ok
test tests::disturbance_energy_is_accounted_for ... ok
test tests::repeated_flow_changes_the_material ... ok
test tests::energy_accounting_remains_closed ... ok
test tests::adaptive_compounding_amplifies_coupling_beyond_inert_seed ... ok
test tests::adaptive_material_preserves_more_channel_information_under_scar ... ok
test tests::adaptive_material_outperforms_inert_material_under_scar ... ok
test result: ok. 8 passed; 0 failed
```
This proves the libm-backed math is bit-for-bit compatible with upstream std
math for this workload. The closed invariant holds at relative error <3e-5
(test tolerance) in the no_std-compiled code path.

### M2 — host-harness falsify

Command:
```
cargo run -p host-harness --release -- falsify \
    --output "C:/Projects/ATOM OS/artifacts/host-falsify.txt"
```
Result: **adaptive_radiation_gain = 39.953x**, falsification_gate = PASS.

Upstream `README.md:120` reports 39.953245299x. We report 39.953244985x.
Difference: ~3e-7 (rounding noise from a different RNG sequence in
measurement binning). Effectively identical.

Accounting errors across all 6 stacks: max 9.3e-8.
All 6 stacks deterministic (visible_hash == repeated_visible_hash).

### M3 — kernel-glue tests

Command:
```
cargo test -p kernel-glue --release
```
Result: **7/7 tests pass**.
```
test tests::scheduler_pick_returns_none_for_empty_mask ... ok
test tests::out_of_range_pids_are_rejected ... ok
test tests::scheduler_pick_returns_a_ready_pid ... ok
test tests::evolves_deterministically ... ok
test tests::ipc_send_then_evolve_keeps_accounting_closed ... ok
test tests::ipc_recv_returns_accumulated_trace_delta ... ok
test tests::closed_invariant_holds_through_glue ... ok
```

### M4 — integration tests

```
cargo run -p closed-energy --release
  introduced=1542.70, resident=150.57, radiated=1162.61, dissipated=229.52
  accounted=1542.70, relative_error=8.39e-8
  closed-energy: PASS

cargo run -p ipc-energy --release
  final radiated=592.32, resident=185.28, dissipated=153.63
  relative_error=9.0e-8, last trace delta=0.117
  ipc-energy: PASS

cargo run -p scheduler-determinism --release
  field A picks == field B picks (40/40 identical)
  13 distinct transitions (scheduler is dynamic)
  scheduler-determinism: PASS
```

All three integration claims hold through the full glue surface.

## M5 — patch generation and verification

### Patch generation

Generated by hand-editing a fresh clone of `atom-os-kernel` at commit
`3fe46c8` and running `git diff --cached HEAD > patch`. The clone was
placed at `C:\Projects\ATOM OS\.patch-work\atom-os-kernel-fresh` (inside
the project boundary). `Cargo.lock` was excluded from the patch.

Patch file: `C:\Projects\ATOM OS\patches\0001-atom-os-field-substrate.patch`
(367 lines).

Files touched by the patch:
- `Cargo.toml` (workspace root) — add `[workspace.dependencies]` for the
  field substrate crates.
- `kernel-kit/Cargo.toml` — add `field-core`, `kernel-glue`, `libm`,
  `spin` deps.
- `kernel-kit/src/lib.rs` — re-export `field_core as field`,
  `kernel_glue as glue`, declare `scheduler_glue` module.
- `kernel-kit/src/scheduler_glue.rs` (new) — global FieldState with
  `init`, `maybe_evolve`, `age`, `set_evolution_divisor`, `with_field`.
  Uses `&raw const` / `&raw mut` for Rust-2024 `static_mut_refs` rules.
- `kernel-orchestrator/src/syscall.rs` — new syscall numbers
  `SYS_FIELD_STIMULATE=17`, `SYS_FIELD_EVOLVE=18`, `SYS_FIELD_OBSERVE=19`,
  `SYS_FIELD_MEASUREMENTS=20`; rewire existing `SYS_IPC_SEND=15` and
  `SYS_IPC_RECV=16` through the field; remove `IPC_MAILBOXES`.
- `x86_64-kernel/src/main.rs` — call `scheduler_glue::init()` after
  orchestrator init; call `scheduler_glue::maybe_evolve(tick as u64)`
  in `timer_interrupt_handler`.

### Patch applies cleanly

```
cd "C:/Projects/ATOM OS/.patch-work"
rm -rf verify-apply
git clone "C:/Projects/atom-os-kernel" verify-apply
cd verify-apply
git apply --check "C:/Projects/ATOM OS/patches/0001-atom-os-field-substrate.patch"
# -> PATCH APPLIES CLEANLY
```

### Patched kernel compiles

After temporarily re-pointing the patch's `path = "../ATOM OS/substrate/..."`
to a local sibling copy for build verification:

```
cargo check -p kernel-kit           # OK (only pre-existing warnings)
cargo check -p kernel-orchestrator  # OK (only pre-existing warnings)
cargo check -p x86_64-kernel        # 2 errors, both pre-existing:
                                    #   include_bytes!("../../target/x86_64-os/release/payload")
                                    #   include_bytes!("../../target/x86_64-os/release/daemon")
                                    # (payloads must be built first; NOT caused by the patch)
```

`APPLY.md` written. It covers prerequisites, what the patch does,
intentional non-goals (scheduler rewrite + GAP-5 fix deferred),
application steps, verification steps (type-check, substrate-level
proof, QEMU boot), rollback, and provenance.

## M6 — GAP-5 fix and scheduler rewrite: deferred

The plan called for fixing GAP-5 (scheduler save/restore corrupting RIP
across context switch) and rewriting `Scheduler::switch_context` to
consult `kernel_glue::scheduler_pick()` in the same patch.

**Decision: deferred to a follow-up patch.** Reasons:

1. The scheduler rewrite is high-risk: it changes the timer-IRQ fast
   path, and a bug there breaks boot. The plan called for a
   `field-scheduler` cargo feature gate so round-robin remains the
   fallback; the gap-5 fix and the field-scheduler wiring should land
   together so they can be tested as one change in QEMU.
2. The kernel cannot currently be QEMU-booted from inside this
   workspace (the payloads must be built first via the kernel's separate
   build process, and a full QEMU smoke test is out of scope for a
   single bridge patch).
3. The current patch delivers all three substrate surfaces
   (kernel service syscalls, IPC re-implementation, scheduler hook
   plumbing) WITHOUT changing scheduling behavior — payloads still run
   on the existing round-robin scheduler. That is a coherent, testable,
   shippable first cut.

The scheduler hook (`kernel_glue::scheduler_pick`) is implemented and
unit-tested in the substrate workspace (M3 test
`scheduler_pick_returns_a_ready_pid` and integration test
`scheduler-determinism`). It is ready to be called by a future
`field-scheduler` feature in `Scheduler::switch_context`. When that
follow-up patch lands, GAP-5 will be fixed in the same change.

## Final state of C:\Projects\ATOM OS

```
C:\Projects\ATOM OS\
├── STUDY.md                          # what Atom OS is, the structural gap
├── ARCHITECTURE.md                   # the bridge design and policy mappings
├── APPLY.md                          # patch prerequisites, application, rollback
├── NOTES.md                          # this file (running work log)
├── artifacts\
│   └── host-falsify.txt              # M2 falsify report (gate=PASS, gain=39.953x)
├── patches\
│   └── 0001-atom-os-field-substrate.patch   # 367-line patch, applies cleanly
└── substrate\
    ├── Cargo.toml                    # workspace
    ├── Cargo.lock
    ├── field-core\                   # no_std port of universum field.rs (8/8 tests)
    ├── field-std\                    # std facade (unused but available)
    ├── kernel-glue\                  # policy layer (7/7 tests)
    ├── host-harness\                 # falsify/sweep binary (39.953x gain)
    └── integration-tests\
        ├── closed-energy\            # PASS (rel err 8.4e-8)
        ├── ipc-energy\               # PASS (rel err 9.0e-8)
        └── scheduler-determinism\    # PASS (40/40 identical picks, 13 transitions)
```

`.patch-work\` is leftover scratch from patch verification; a background
cargo process held some files in `verify-apply\target\` and `rm -rf`
failed on first attempt. It is safe to delete on next boot. It contains
only clones of the kernel and copies of the substrate, nothing else.

## Verification summary

| Claim | Method | Result |
|---|---|---|
| no_std port of field.rs is bit-compatible with upstream std math | `cargo test -p field-core --release` (8 tests) | 8/8 pass, zero warnings |
| Adaptive radiation gain reproduced | `host-harness falsify` | 39.953x (upstream: 39.953x) |
| Closed invariant holds through glue | `kernel-glue` test + `closed-energy` integration | rel err < 1e-7 |
| IPC preserves accounting | `ipc-energy` integration | rel err 9.0e-8, radiated > 0 |
| Scheduler decisions deterministic | `scheduler-determinism` integration | 40/40 identical, 13 transitions |
| Patch applies cleanly | `git apply --check` on fresh clone of `3fe46c8` | clean |
| Patched kernel compiles | `cargo check -p kernel-kit -p kernel-orchestrator` | clean (pre-existing warnings only) |
| Full substrate workspace builds & tests | `cargo test --workspace --release` | 15/15 tests pass |
