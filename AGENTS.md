# Disk II peripheral

Follow the umbrella PRINCIPLES.md and Emu198x/emu198x/RULES.md. Standalone
peripheral under workspace-shape.md; no machine or file-format dependencies.
The approved plan is docs/plans/2026-11-04-disk-ii.md (copied from apple-ii).
No firmware may be committed. P5/P6 PROMs and media remain external fixtures.
One host crystal tick drives time; Q3 falling drives the sequencer. Never add
clocks inside a bus method. Archive media defaults write-protected; explicit
writable work media changes RAM only. Keep source files immutable.
Verify fmt, Clippy with warnings denied, tests, coverage and reference traces.
