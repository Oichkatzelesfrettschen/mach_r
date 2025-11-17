# Active Sprints & Immediate TODOs

This section outlines the current and near-term development focus for Mach_R, organized into sprints. It reflects the most granular and actionable tasks derived from the long-term roadmap, prioritizing correctness first (IPC/VM), then servers and POSIX, with solid tooling.

## Completed
- CI for fmt/clippy/tests; artifact uploads; mdBook build. [done]
- xtask as primary runner; docs updated; MANIFEST.json produced. [done]
- README/AGENTS cross-links; docs: GDB, ARCHITECTURE, STRUCTURE, CLEAN_ROOM. [done]

## Sprint 1 (Immediate Focus)
- IPC: finalize rights transitions (move/make/copy/send-once) and tests.
- Notifications: implement dead-name + no-senders delivery semantics.
- MIG: add descriptor marshalling (Port/OOL/OOL-Ports) and small Name Server RPC example.
- VM: define vm_prot/vm_inherit enums; add default pager mock and page-in unit test.
- Images: xtask `disk-image --with-sysroot` (Linux CI) to populate filesystem; update CI.

## Sprint 2 (Near-Term Focus)
- Scheduler: priority queues + aging; preemption boundary tests.
- Boot/Arch: refine AArch64 MMU/TTBR; unify UART/panic across arch; x86_64 ELF build stub.
- POSIX shim: basic syscall translation for open/read/write/close/exit.
- File server MVP: in-memory FS with port API and tests.

## Sprint 3 (Mid-Term Focus)
- Shared memory + COW; fork semantics tests.
- Signals via exception ports; basic handlers.
- Device server façade (virtio stubs); network loopback skeleton.
- MIG defs parser (reference-only) to emit Rust stubs; xtask integration.

## Operational Tasks
- Add QEMU smoke boot test to CI (serial log capture).
- Property tests for IPC marshalling and rights.
- Release pipeline: changelog generation, versioned artifacts, signed checksums.

## Notes
- See [Long-Term Roadmap](long_term_roadmap.md) for the full 30-item plan.
- All OSFMK references live under `archive/osfmk/reference` (clean-room only).
