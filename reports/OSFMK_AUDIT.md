# OSFMK Audit Report

Source root: /Users/eirikr/OSFMK

Total files: 50

## By Extension

- .tar: 17
- .gz: 16
- .z: 9
- .bz2: 4
- <none>: 1
- .json: 1
- .md: 1
- .zip: 1

## Key Directories


## Suggested Translation Map

- osfmk/ipc ->
  - synthesis/src/port.rs
  - synthesis/src/message.rs
  - synthesis/src/syscall.rs
- osfmk/vm ->
  - synthesis/src/paging.rs
  - synthesis/src/memory.rs
  - synthesis/src/external_pager.rs
- osfmk/kern ->
  - synthesis/src/task.rs
  - synthesis/src/scheduler.rs
  - synthesis/src/interrupt.rs
- osfmk/arm ->
  - synthesis/src/arch/aarch64.rs
  - synthesis/src/arch/mod.rs
- osfmk/i386 ->
  - synthesis/src/arch/x86_64/mod.rs

## Next Steps

- Prioritize vm/, ipc/, kern/ for translation to Rust modules.
- Extract constants and structure layouts; recreate in Rust types.
- Build unit tests around rights transitions, vm faults, and scheduling.