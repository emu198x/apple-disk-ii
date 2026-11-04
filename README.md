# emu198x-apple-disk-ii

[![CI](https://github.com/emu198x/apple-disk-ii/actions/workflows/ci.yml/badge.svg)](https://github.com/emu198x/apple-disk-ii/actions/workflows/ci.yml)

Disk II controller, externally supplied P5/P6 PROMs and two bitstream drives.
No dependencies, ROMs or disk-image formats. Host ticks supply crystal time and
actual Q3 falling edges. Bus selection never advances the controller clock.

The peripheral models soft switches, the PROM sequencer, both drive selects,
quarter-track equilibrium stepping, continuous rotation, write protection and
one-second delayed motor-off. A work mount can write in-memory bit cells;
source-protected disks remain protected. No filesystem access or persistence.

This is a digital bitstream model: instantaneous equilibrium stepping and the
WOZ-recommended MC3470 noise/latency approximation do not qualify analogue drive
acceleration, head settling, pulse shape or arbitrary flux copy protections.

Sources: Sather *Understanding the Apple II*, chapter 9; the primary library's
`apple-ii-disk-ii-teaching.md` and `woz-2-bitstream-format.md`; MAME snapshot
9391e1eff80eb04abd58d48218a1d3b9643c1ea8, wozfdc.cpp and floppy.cpp.

Writes normalize each inter-transition interval to the nearest media cell.
This preserves ordinary DOS writes at the Apple II CPU rate. It does not retain
sub-cell flux timing or exact write splices; copy-protection writes are unqualified.

`cargo test` runs the fixture-free checks. To compare the real P6 PROM against
1,024 independently executed MAME transitions, set `EMU198X_DISK_II_P6` and run
`cargo test --release -- --ignored`. `tools/reference-trace.py` regenerates the
CSV from the pinned MAME source and a caller-supplied P6; firmware stays external.
