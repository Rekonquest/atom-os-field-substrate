# Applying the Atom OS Field Substrate Patch

This document describes how to apply `patches/0001-atom-os-field-substrate.patch`
to a clean clone of `atom-os-kernel`, what the patch does, and how to verify
the patched kernel boots in QEMU.

## Prerequisites

- A clean clone of `atom-os-kernel` at commit `3fe46c8` ("Merge pull request
  #2 from Lucerna-Labs/feature/syscall-infrastructure", 2026-07-17). The
  patch was generated against this commit; applying it to any other commit
  may produce conflicts.
- The ATOM OS substrate workspace at `C:\Projects\ATOM OS\substrate\`. The
  patch references this location via relative path. If your ATOM OS lives
  elsewhere, edit the paths in `Cargo.toml` and `kernel-kit/Cargo.toml`
  after applying (see "Path adjustments" below).
- Rust toolchain with nightly, edition 2024, and the components listed in
  `atom-os-kernel/.cargo/config.toml` (`build-std = ["core", "alloc",
  "compiler_builtins"]`, json-target-spec). The kernel's own toolchain
  requirements apply unchanged; the patch adds no new toolchain
  requirements.
- QEMU (only if you want to boot-test; the patch's correctness can be
  verified with `cargo check` without QEMU).

## What the patch does

1. **`Cargo.toml` (workspace root)** — adds a `[workspace.dependencies]`
   section with `field-core`, `kernel-glue`, `libm`, and `spin` as path
   dependencies pointing at `../ATOM OS/substrate/...`.
2. **`kernel-kit/Cargo.toml`** — adds the same four dependencies so
   `kernel-kit` can `extern crate` them.
3. **`kernel-kit/src/lib.rs`** — re-exports `field_core as field` and
   `kernel_glue as glue`, and declares a new module `scheduler_glue`.
4. **`kernel-kit/src/scheduler_glue.rs` (new file)** — owns the global
   `FieldState` static, lazily initialised at boot, advanced once every
   `EVOLUTION_DIVISOR` ticks (default 10) from the timer IRQ. Exposes
   `init`, `maybe_evolve`, `age`, `set_evolution_divisor`, `with_field`.
   Uses `&raw const` / `&raw mut` throughout to comply with Rust-2024
   `static_mut_refs` rules (the kernel already uses this idiom for
   `SYSTEM`).
5. **`kernel-orchestrator/src/syscall.rs`** — adds four new syscall
   numbers and their handlers:
   - `SYS_FIELD_STIMULATE = 17` — inject energy at a PID's site.
   - `SYS_FIELD_EVOLVE = 18` — advance the field `moments` moments.
   - `SYS_FIELD_OBSERVE = 19` — read the trace-peak delta for a PID.
   - `SYS_FIELD_MEASUREMENTS = 20` — read the field's `introduced` total.
   The existing `SYS_IPC_SEND = 15` and `SYS_IPC_RECV = 16` are rewired
   through the field (their syscall numbers are preserved so existing
   payloads still link):
   - SEND injects energy at the sender biased toward the target via
     `kernel_glue::ipc_send_as_energy`. The byte payload is converted to
     a magnitude (0.02/byte, capped at 5.0).
   - RECV reads the accumulated radiation delta for the calling PID via
     `kernel_glue::ipc_recv_as_radiation`; if above a threshold it maps a
     page at `0x300000` containing the magnitude as ASCII.
   The flat `IPC_MAILBOXES` array is removed.
6. **`x86_64-kernel/src/main.rs`** — calls
   `kernel_kit::scheduler_glue::init()` after the orchestrator is up
   (prints "Field substrate Initialized."), and calls
   `kernel_kit::scheduler_glue::maybe_evolve(tick as u64)` at the top of
   `timer_interrupt_handler`.

## Out of scope (intentionally NOT in this patch)

- **Scheduler rewrite**. The round-robin scheduler in
  `kernel-orchestrator/src/scheduler.rs` is unchanged. The
  `kernel_glue::scheduler_pick()` hook exists and is unit-tested, but the
  scheduler does not yet consult it. Promoting the scheduler to
  field-driven is feature-gated work tracked separately. See M6 / future
  patches.
- **GAP-5 fix**. The known RIP-corruption bug in `switch_context` is
  unchanged. The plan called for fixing GAP-5 alongside the scheduler
  rewrite; both are deferred.
- **Spiderweb Bus**. The std-only bus at `C:\Projects\kernel Os-spiderwebBus`
  is not ported.

## Path adjustments (if ATOM OS is not at C:\Projects\ATOM OS)

After applying the patch, edit two files:

1. `Cargo.toml` (workspace root) — `[workspace.dependencies]` section.
2. `kernel-kit/Cargo.toml` — `[dependencies]` section.

Change the `path = "../ATOM OS/substrate/..."` and
`path = "../../ATOM OS/substrate/..."` entries to the correct relative
paths from your kernel clone. Spaces in the path (`ATOM OS`) are fine in
TOML strings.

## Application

From a clean clone of `atom-os-kernel`:

```bash
cd /path/to/atom-os-kernel
git status                                # working tree must be clean
git rev-parse HEAD                        # confirm 3fe46c8 (or a close descendant)
git apply --check /path/to/ATOM\ OS/patches/0001-atom-os-field-substrate.patch
git apply         /path/to/ATOM\ OS/patches/0001-atom-os-field-substrate.patch
```

If `git apply --check` succeeds, the patch will apply cleanly. If it
fails, your tree has diverged from `3fe46c8`; either rebase to that
commit or apply by hand using the patch as a guide.

## Verification

### 1. Type-check (no QEMU needed)

```bash
cargo check -p kernel-kit
cargo check -p kernel-orchestrator
```

Expected result: clean compile (only pre-existing kernel warnings such as
unused `mut`).

### 2. Substrate-level proof (no QEMU needed)

Independent of the kernel, the substrate workspace proves the physics:

```bash
cd /path/to/ATOM\ OS/substrate
cargo test -p field-core --release        # 8/8 universum tests pass
cargo test -p kernel-glue --release       # 7/7 glue tests pass
cargo run -p host-harness --release -- falsify --output artifacts/host-falsify.txt
                                           # adaptive_radiation_gain ~ 39.95x
```

### 3. Boot in QEMU (full kernel)

Requires the payloads (`shell.elf`, `daemon.elf`) to be built first per
the kernel's own build process (`make payload daemon` or equivalent).
Then:

```bash
cargo bootimage run
```

Expected boot banner sequence (in the kernel's VGA output):

```
Booting Fearless Hypatia...
... (existing boot messages) ...
Orchestrator Initialized.
Field substrate Initialized.        <- added by this patch
... (existing PIC / GDT / TSS messages) ...
```

The kernel's existing benchmark payload should still run. The new
syscalls (17-20) are available to any new payload.

## Rollback

```bash
cd /path/to/atom-os-kernel
git apply --reverse /path/to/ATOM\ OS/patches/0001-atom-os-field-substrate.patch
# or, if you have committed the change:
git revert <commit-hash>
```

The patch only adds files (`kernel-kit/src/scheduler_glue.rs`) and edits
four existing files (`Cargo.toml`, `kernel-kit/Cargo.toml`,
`kernel-kit/src/lib.rs`, `kernel-orchestrator/src/syscall.rs`,
`x86_64-kernel/src/main.rs`). Rollback restores all four and deletes the
new file.

## Patch provenance

- **Base commit**: `3fe46c8 Merge pull request #2 ...` (2026-07-17).
- **Generated**: 2026-07-20 by diffing a working clone against its HEAD
  after applying the bridge changes by hand.
- **Verified to apply cleanly** to a fresh clone of `3fe46c8` on the same
  day.
- **Verified to compile** (kernel-kit, kernel-orchestrator) when the
  substrate path deps resolve.
