# Changelog

## [0.1.0] - 2026-11-04

### Added

- Execute caller-supplied Disk II P5/P6 PROMs from host crystal/Q3 clocks.
- Read two bitstream drives with quarter-track stepping, write protection and
  one-second delayed motor-off. Bus reads and peeks never advance time.
- Write explicitly writable memory tracks using normalized transition intervals.
  Original Apple II DOS 3.3 saves and reloads an Integer BASIC program; the
  exported WOZ reloads in a fresh machine. Sub-cell flux writes are unqualified.
