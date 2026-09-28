# Changelog

## 1.1.0 - 2026-09-28

### Added
- A help popup for every plot, explaining what is plotted and how to read it.
  Toggle it with `?` or `h`.
- Per-representation timing in the phonon chart, coloured from green to red by the
  time each iteration took.
- The phonon convergence threshold, with the current `|ddv_scf|²` beside it, in the
  header panel.
- The pressure plot distinguishes positive from negative pressure by colour and
  marker, which the logarithmic axis would otherwise hide.
- The version is shown in the title bar.

### Fixed
- LSDA runs report the k-points of one spin channel only, so a spin-polarised nscf
  showed half the work as the whole job and read as finished at the midpoint.
- The number of q-points was misread in certain phonon runs.

## 1.0.1 - 2026-09-16

### Fixed
- nscf runs using k-point pools (`pw.x -nk`) reported the per-pool k-point count as
  though it were the whole calculation. Pools and total k-points are now correctly reported.

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
