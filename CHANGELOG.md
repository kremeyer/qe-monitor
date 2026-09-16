# Changelog

## 1.0.0 - 2026-09-16

### Added
- Wannier90 support: disentanglement and spread convergence plots, per-Wannier-function
  spreads, and a summary panel with iteration counts and timings.
- Two independently selectable plot panels - function keys pick the left one, number
  keys the right.
- Estimated time remaining during disentanglement, marked `+ WANN` to show that
  wannierisation still follows.
- Integration tests running against real Quantum ESPRESSO 7.5 and Wannier90 3.1.0
  output.

### Fixed
- EPW appends each wannier90 run to the same `.wout`; only the most recent run is
  parsed now, instead of concatenating the iteration series of several runs.
- `.wout` files that EPW appended without re-emitting the banner, starting straight at
  `Resuming Wannier90`, were rejected as unrecognised.
- ph.x counted every q-point twice in electron-phonon runs, reporting progress such as
  16/8.
- ph.x q-point and irreducible-representation counts in runs split over q-points, and
  in recovered runs.
- Average time per SCF iteration and per k-point were divided by the step count rather
  than the number of intervals between timings.

### Changed
- Split into a library and a binary target, so the parsers can be tested directly.

## 0.3.0 and earlier

See the git history.
