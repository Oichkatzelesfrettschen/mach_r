# real_os (experimental)

Prototype workspace for a split bootloader+kernel layout and experiments
with alternative build flows.

- Mainline kernel development is in `../` (crate `mach_r`).
- Treat this area as a sandbox; avoid coupling or cross-imports.
- Document any lessons learned in `synthesis/docs/` and update
  stable build targets when changes mature.

See `../../AGENTS.md` for repository conventions and practices.

