# OSFMK → Mach_R Translation Plan

Scope
- Translate Mach OSF/CMU subsystems into safe Rust equivalents in `synthesis/`.
- Use OSFMK archives (see `archive/osfmk/archives_summary.json`) as reference only.

Mapping
- IPC (osfmk/ipc) → `port.rs`, `message.rs`, `syscall.rs`
- VM  (osfmk/vm)  → `paging.rs`, `memory.rs`, `external_pager.rs`
- Kern (osfmk/kern) → `task.rs`, `scheduler.rs`, `interrupt.rs`
- Arch (osfmk/arm,i386) → `arch/aarch64/*`, `arch/x86_64/*`

Milestones
- M1: VM primitives parity (map/unmap, faults, protections).
- M2: IPC rights transitions + dead-name notifications.
- M3: Scheduler run queues + preemption model.
- M4: Boot sequence stabilization with UART logging.

Immediate Actions
- Extract constants/struct layouts from headers and recreate in Rust types.
- Add unit tests mirroring OSFMK invariants (ports, rights, VM faults).
- Document equivalents in `docs/ARCHITECTURE.md` and `docs/book`.

Notes
- Do not copy OSFMK source into Rust; translate interfaces and behaviors.
- Track progress as issues linked to these milestones.

