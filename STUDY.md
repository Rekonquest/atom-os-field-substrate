# Atom OS — Field Substrate Study

This document is the verified ground-truth synthesis from the study session of
2026-07-20. It records what the Atom OS family actually is today, where the
universe-mimicking substrate lives, and the structural gap this project exists
to close. All claims are grounded in first-hand reads of the cited files; any
inference is labelled as such.

## 1. What "Atom OS" actually is today

The real kernel lives at `C:\Projects\atom-os-kernel` (NOT this directory —
this directory, `C:\Projects\ATOM OS`, is the integration workspace created by
this project and was empty before this session began).

- **Rust `no_std` x86_64 kernel**, codename "Fearless Hypatia", boots in QEMU
  via `bootloader = "0.9.23"`.
- Entry point: `x86_64-kernel/src/main.rs:661` (`_start(boot_info)`).
- Last verified commit: `3fe46c8` 2026-07-17 ("Merge PR #2 feature/syscall-infrastructure").
- Workspace members: `daemon`, `kernel-kit`, `kernel-orchestrator`, `payload`,
  `x86_64-kernel`.

### Core abstractions present today

- **Scheduler** (`kernel-orchestrator/src/scheduler.rs`): fixed `MAX_TASKS = 16`
  slot array, preemptive round-robin, context switch by swapping `rsp` only.
  Known bug **GAP-5**: timer save/restore path corrupts RIP across a context
  switch (documented in the kernel's own `NOTES.md`).
- **Memory** (`kernel-kit/src/memory.rs`, `slab.rs`, `paging.rs`): `SlabLocked`
  allocator with `BumpAllocator` fallback on a 16 MiB `.bss` heap; 4-level
  paging with `duplicate_pml4`, `map_segment`, `phys_to_virt`/`virt_to_phys`.
- **IPC / bus** (`kernel-orchestrator/src/syscall.rs:332-378`): a flat global
  array `pub static mut IPC_MAILBOXES: [Option<Vec<u8>>; 16]`, one slot per
  PID. `SYS_IPC_SEND=15` writes into `target_pid`'s mailbox. `SYS_IPC_RECV=16`
  copies the message into a freshly-mapped page at `0x300000` in the receiver's
  address space. This is a trivial untyped mailbox, not a typed pub/sub bus.
- **Syscalls** (`kernel-orchestrator/src/syscall.rs:6-19`): 17 syscalls,
  YIELD(1), ALLOC(2), EXIT(3), READ(4), WRITE(5), OPEN(6), READ_FILE(7),
  WRITE_FILE(8), CLOSE(9), LIST_DIR(10), CLEAR(11), TRUNCATE(12), EXEC(13),
  PRINT(14), IPC_SEND(15), IPC_RECV(16). Dispatcher uses the mechanical
  `compare` atom (not `==`).
- **Atoms** (`kernel-kit/src/atoms.rs`): 8 primitives — `scan, hash, fold,
  project, scale, compare, combine, order`. Only 5 are actually used by the
  kernel today; the design doc calls the rest "GAP territory".

### The Spiderweb Bus is NOT in the kernel

`C:\Projects\kernel Os-spiderwebBus` is a mature **std-only** library (15
crates, 4,092-line core, `forbid(unsafe_code)`, last touched 2026-07-01) with
rich vocabulary — `Strand`, `Socket`, `Bus`, `Fabric`, `Vibration`, `Spider`,
`Highway`, `Lanes`, plus a `spiderweb-heat` crate applying the heat kernel
`H_t = exp(−tL)`. It cannot link into the `no_std` kernel today and grep found
zero references to "spiderweb/strand/fabric" inside `atom-os-kernel`. The
kernel uses a flat mailbox IPC instead. They are currently unrelated projects.

## 2. The universe-mimicking substrate

**`C:\Projects\universe equavelant to vulkan driver`** — internal crate name
`universum-substrate`. Despite the folder name it is **not a Vulkan port**. It
is a bounded 2D material-field cellular physics substrate (Rust, edition 2024,
last file mod 2026-07-19). The `README.md` is explicit:

> "This project does not translate Vulkan into smaller operations. It starts
> with a material field and asks whether stable, useful organization can
> emerge from local physical laws under an observation boundary."

### Physics laws (in `src/field.rs`)

The load-bearing file is `src/field.rs` (1,102 lines). All default constants
are operator-chosen and clamped to `[0,1]`:

| Law | Site of definition | Default constant |
|---|---|---|
| Local exchange (relaxation + drift) | `exchange`, lines 675-713 | `diffusion=0.105`, `gradient=0.24` |
| Conductance (material × alignment × relay) | `exchange`, lines 651-656 | — |
| **Radiation (emission)** | `evolve`, lines 328-331 | **`radiation=0.19`** |
| **Dissipation (decay)** | `evolve`, lines 333-335 | **`dissipation=0.0012`** |
| Permeability formation/erosion | `evolve`, lines 337-341 | `formation=0.022`, `erosion=0.0018` |
| Adaptive coupling (Hebbian) | `adapt_coupling`, lines 730-769 | `coupling_formation=0.220`, `coupling_erosion=0.00035` |
| Phase relay + relay guard | `exchange` line 654, `relay_guard` lines 671-685 | `phase_relay=0.0`, `relay_guard=0.0` |

**Closed-system invariant**: `introduced = resident + radiated + dissipated`.
The test `energy_accounting_remains_closed` (`field.rs:933-950`) enforces
relative error < 3e-5 over 500 moments.

**Falsification stack order** (`src/falsify.rs:5`):
`boundary_stimulation → local_energy_flow → permeability_formation → erosion
→ spectral_coupling → radiation → dissipation`.

Radiation and dissipation are the **top two primitives** — exactly the concept
the operator described as "mimic the universe using radiation, dissipation
etc." Reported adaptive radiation gain: **39.95×** over inert material
(`README.md:120`).

### Dependencies (verified from `Cargo.toml` and source)

- `std::f32::consts::{PI, TAU}` — trivially replaceable with constants.
- `Vec` — replaceable with `alloc::vec::Vec` under `no_std + alloc`.
- `std::ops::{Add, AddAssign, Sub, SubAssign, Mul}` — all in `core::ops`.
- `f32` transcendentals: `sin`, `cos`, `ln_1p`, `powf`, `atan2`, `log2`,
  `sqrt`, `rem_euclid`, `fract`, `clamp`, `abs`, `min`, `max`. Of these,
  `abs`/`min`/`max`/`clamp`/`rem_euclid`/`fract`/`sqrt` are in `core`; the
  transcendentals (`sin`, `cos`, `ln_1p`, `powf`, `atan2`, `log2`) require
  `libm` for the no_std port.

## 3. The structural gap this project closes

Today the universe-mimicry substrate and the Atom OS kernel are **completely
disconnected**. Universum-substrate is a standalone userspace simulator;
`atom-os-kernel` has zero physics code and uses no radiation/dissipation
primitives. No source file in either project references the other.

This project bridges them by:
1. Porting the field into a `no_std` crate inside this workspace.
2. Mapping field state onto OS concepts (energy ↔ attention, coupling ↔ IPC
   affinity, permeability ↔ wakefulness, radiation ↔ output, dissipation ↔
   forgetting, exchange ↔ priority/message flow).
3. Exposing the field as a kernel service via new syscalls
   (`SYS_FIELD_STIMULATE`, `SYS_FIELD_EVOLVE`, `SYS_FIELD_OBSERVE`,
   `SYS_FIELD_MEASUREMENTS`).
4. Re-implementing `SYS_IPC_SEND`/`RECV` on top of the field (syscall numbers
   preserved so existing payloads still link).
5. Replacing the round-robin scheduler with field-driven selection and fixing
   GAP-5 along the way.

The kernel-side changes ship as a generated patch and `APPLY.md` inside this
workspace; the `atom-os-kernel` files themselves are not modified without
explicit operator approval.

## 4. Source provenance

| File | Lines | Last modified |
|---|---|---|
| `C:\Projects\universe equavelant to vulkan driver\src\field.rs` | 1,102 | 2026-07-19 |
| `C:\Projects\universe equavelant to vulkan driver\src\falsify.rs` | 477 | 2026-07-19 |
| `C:\Projects\universe equavelant to vulkan driver\src\render.rs` | 72 | 2026-07-19 |
| `C:\Projects\universe equavelant to vulkan driver\src\lib.rs` | 19 | 2026-07-19 |
| `C:\Projects\universe equavelant to vulkan driver\src\bin\falsify.rs` | 344 | 2026-07-19 |
| `C:\Projects\atom-os-kernel\kernel-orchestrator\src\syscall.rs` | 378 | 2026-07-17 |
| `C:\Projects\atom-os-kernel\kernel-orchestrator\src\scheduler.rs` | 102 | 2026-07-17 |
| `C:\Projects\atom-os-kernel\kernel-kit\src\atoms.rs` | 72 | 2026-07-17 |

## 5. Out of scope

- Porting the Spiderweb Bus to `no_std` (separate effort; would be a parallel
  bridge into the kernel).
- Building out any of the ~42 empty `Atom *` placeholder folders under
  `C:\Projects`.
- Modifying `universum-substrate` itself (read-only source for this project).
- GPU acceleration of the field. `atom-gpu` exists but is unrelated; this
  project is CPU-only.
