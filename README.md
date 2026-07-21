# ATOM OS Field Substrate Bridge

[![CI Tests](https://github.com/Rekonquest/atom-os-field-substrate/actions/workflows/test.yml/badge.svg)](https://github.com/Rekonquest/atom-os-field-substrate/actions/workflows/test.yml)

This repository contains the **ATOM OS Field Substrate Bridge** project, which integrates the universum radiation/dissipation field simulation as the computational substrate for the Atom OS kernel.

## Overview

The project bridges the universe-mimicking substrate from `universum-substrate` into the Atom OS kernel (`atom-os-kernel`), making the field the single computational substrate serving three roles:

1. **Kernel service** — Field evolves one moment per timer tick, observable through new syscalls
2. **IPC substrate** — `SYS_IPC_SEND`/`SYS_IPC_RECV` re-implemented as spectral-energy propagation
3. **Scheduler** — Field-driven selection replaces round-robin (feature-gated, deferred to follow-up patch)

### Field-State to OS-State Mapping

| Field State | OS Meaning |
|-------------|------------|
| `site.energy` (RGB) | Process ready-state / attention |
| `site.coupling` | IPC affinity — energy flows where coupling is high |
| `site.permeability` | Wakefulness / memory retention |
| `site.trace` | Output emitted to observer (logs, syscall results) |
| `radiation` law | Process emitting visible output |
| `dissipation` law | Idle decay — quiet process forgets |
| `exchange` law | Priority propagation + IPC message flow |

## Repository Structure

```
.
├── STUDY.md                 # Ground truth synthesis
├── ARCHITECTURE.md           # Bridge design and policy mappings
├── APPLY.md                 # Patch application instructions
├── NOTES.md                 # Work log with verification results
├── patches/
│   └── 0001-atom-os-field-substrate.patch  # Kernel patch
├── artifacts/
│   └── host-falsify.txt      # Falsification report
└── substrate/
    ├── Cargo.toml            # Workspace manifest
    ├── field-core/           # no_std + alloc port of universum field.rs
    ├── field-std/            # std facade for host testing
    ├── kernel-glue/          # Policy layer: field ↔ kernel mapping
    ├── host-harness/         # Falsify/sweep binary
    └── integration-tests/    # Closed-energy, IPC-energy, scheduler-determinism
```

## Testing

### Local Testing (Windows)

Run the quick test suite from PowerShell:

```powershell
# From repository root
cd substrate
cargo test --workspace --release

# Run individual integration tests
cargo run -p closed-energy --release
cargo run -p ipc-energy --release  
cargo run -p scheduler-determinism --release

# Run falsification harness
cargo run -p host-harness --release -- falsify
```

Or use the provided script:
```powershell
\scripts\test-quick.ps1
```

### Local Testing (Linux/macOS)

```bash
# From repository root
cd substrate
cargo test --workspace --release

# Run all tests via script
../scripts/test-all.sh
```

### GitHub Actions

All tests run automatically on push via `.github/workflows/test.yml`:
- Workspace unit tests (15 total)
- Integration tests (3 total)
- Host harness falsification

### GitHub Codespaces

This repository includes a devcontainer configuration for GitHub Codespaces:

1. **Create a Codespace:**
   - Go to [github.com/Rekonquest/atom-os-field-substrate](https://github.com/Rekonquest/atom-os-field-substrate)
   - Click **Code** → **Codespaces** tab
   - Click **Create codespace on main**

2. **The Codespace includes:**
   - Rust nightly toolchain
   - libm-dev for no_std transcendentals
   - GitHub CLI
   - QEMU (for future kernel testing)
   - Pre-configured VS Code extensions

3. **Run tests in Codespace:**
   ```bash
   cd substrate
   cargo test --workspace --release
   ```

## Verification Status

All milestones completed and verified:

| Milestone | Status | Result |
|----------|--------|--------|
| M1 field-core | ✅ | 8/8 tests pass, closed invariant holds |
| M2 host-harness | ✅ | 39.953x adaptive radiation gain |
| M3 kernel-glue | ✅ | 7/7 tests pass |
| M4 integration tests | ✅ | All 3 pass (rel err < 1e-7) |
| M5 patch | ✅ | Applies cleanly to atom-os-kernel@3fe46c8 |
| M6 docs | ✅ | Complete documentation |

**Total: 15/15 tests pass, all integration claims verified**

## Patch Application

The generated patch file at `patches/0001-atom-os-field-substrate.patch` applies to `atom-os-kernel` at commit `3fe46c8`. See `APPLY.md` for detailed instructions.

### Quick Patch Test

```bash
# From atom-os-kernel repository
cd /path/to/atom-os-kernel
git apply --check /path/to/atom-os-field-substrate/patches/0001-atom-os-field-substrate.patch
# If clean, apply:
git apply /path/to/atom-os-field-substrate/patches/0001-atom-os-field-substrate.patch

# Verify compilation
cargo check -p kernel-kit
cargo check -p kernel-orchestrator
```

## Deferred Work

The following items are intentionally deferred to a follow-up patch:

1. **Scheduler rewrite** — Replace round-robin with field-driven `scheduler_pick()`
2. **GAP-5 fix** — Fix RIP corruption bug in context switch

These are gated behind a `field-scheduler` cargo feature and will land together in a future patch after QEMU smoke testing.

## Closed Invariant

The field's defining property is the closed invariant:

```
introduced = resident + radiated + dissipated
```

Enforced to relative error < 3e-5 over 500 moments. This invariant becomes the OS-wide accounting guarantee when the field serves as the computational substrate.

## License

MIT OR Apache-2.0
