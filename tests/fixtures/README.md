# Test fixtures

Real output files, used as test input. Nearly all are **QE 7.5 / Wannier90 3.1.0**
— the versions the tool is actually pointed at in practice. Older formats were
deliberately excluded: this is a live-monitoring tool, and QE renamed
`convergence threshold` to `scf convergence threshold` after 6.1, so pre-7.x
output does not exercise the current parser paths.

**Licensing:** everything except the three files noted below is output generated
locally and is BSD-licensed like the rest of the repo. See `NOTICE` at the repo
root.

**Do not edit these files.** Their value is that they are what the programs
actually emit. If a test needs a different shape, add a fixture and record it here.

## How the QE fixtures were generated

The `pw/`, `ph/` and `epw/` fixtures come from Quantum ESPRESSO's own test-suite,
run locally against QE 7.5 rather than taken from the stale reference outputs that
ship with it (those were captured around v6.0/6.1 in 2017):

```sh
cd $QE/test-suite
make run-tests-pw  NPROCS=8
make run-tests-ph  NPROCS=8
make run-tests-epw NPROCS=8
```

That writes `test.out.<DDMMYY>.inp=<input>` next to each `benchmark.out.git.inp=*`
reference. The `test.out.*` files are what was copied here, renamed for legibility.

All are MPI runs with 1 OpenMP thread, so every one exercises the
`Number of MPI processes:` / `Threads/MPI process:` branch of `parse_run_info`
(which had no coverage before). Note the rank counts differ by group:

| Group | MPI ranks | OpenMP threads |
|---|---:|---:|
| `pw/` | 8 | 1 |
| `ph/` | 12 | 1 |

EPW-written `.wout` files carry no `Running in serial/parallel` line at all, so
`mpi_ranks` is `None` for those — expected, not a parsing gap. The standalone
wannier90 fixture does have one (`Running in serial`), giving `Some("1")`.

Test systems are QE's own (Si, Al, SiC, BAs, BN, diamond) — no local research data.

## `pw/` — pw.x, QE 7.5

| File | Bytes | calc | scf_conv | etot/forc/press | blocks | ion-dyn | Source test |
|---|---:|---|---|---|---:|---:|---|
| `scf.out` | 9106 | Scf | 1e-6 | – | 1 | 0 | `pw_scf` / `scf.in` |
| `nscf_bands.out` | 8532 | **Nscf** | – | – | 0 | 0 | `pw_scf` / `scf-2.in` |
| `metal_smearing.out` | 10524 | Scf | 1e-6 | – | 1 | 0 | `pw_metal` / `metal.in` |
| `relax.out` | 32881 | Scf | 1e-7 | 1e-4 / 1e-3 / – | 5 | 5 | `pw_relax` / `relax.in` |
| `vc_relax.out` | 67113 | Scf | 1e-9 | 1e-4 / 1e-3 / 0.5 | 11 | 10 | `pw_vc-relax` / `vc-relax4.in` |

`nscf_bands.out` is the only fixture carrying the `Band Structure Calculation`
marker that `src/pw/parser.rs` keys on to select `PwCalcType::Nscf`.

`relax.out` and `vc_relax.out` are the only ones with the convergence thresholds
that `is_relax` (see `src/ui.rs` and `src/pw/ui.rs`) uses to decide whether to show
the Force/Pressure tabs and the target-vs-current threshold panel. `vc_relax.out`
is the only one with a pressure threshold.

## `ph/` — ph.x, QE 7.5

| File | Bytes | q-points | irreps | scf blocks | Source test |
|---|---:|---:|---|---:|---|
| `base_si.out` | 26916 | 1 | [3] | 3 | `ph_base` / `si.phX.in` |
| `metal_multiq.out` | 99120 | **8** | [1,2,2,2,3,3,2,2] | 17 | `ph_metal` / `al.elph.in` |
| `restart1-4.out` | ~10700 ea. | 1 | [2] | 0–1 | `ph_restart` / `SiC.phG.restart<N>.in` |

`metal_multiq.out` is an **electron-phonon** run. Note that ph.x writes
`Number of q in the star` *twice* per q-point in an el-ph calculation — once after
the dynamical matrix and once after the λ/γ section — so a naive count of that
marker reports double. `Diagonalizing the dynamical matrix` appears exactly once
per completed q-point and is the reliable marker.

`restart1-4.out` are a restart series: each resumes a partially completed run, so
they have few or no completed scf blocks.

## `epw/` — wannier90 3.1.0, written by EPW in library mode

EPW appends each wannier90 invocation to the same `.wout`, so one file can hold
several runs back to back. `Resuming Wannier90` appears *inside* a run (EPW calls
`wannier_setup`, then `wannier_run`) and does not start a new one.

| File | Bytes | Runs | DIS | spread | WF | Source test |
|---|---:|---:|---:|---:|---:|---|
| `bas_multirun.wout` | 84911 | **2** | 3 | 16 | 8 | `epw_wfpt` / BAs |
| `si_disentangle.wout` | 62125 | 1 | 17 | 4 | 16 | `epw_mob` / Si |
| `bn_no_disentangle.wout` | 36685 | 1 | 0 | 5 | 3 | `epw_hall` / BN |

`bas_multirun.wout` is the canonical multi-run case: a parser that does not
restrict itself to the final run will concatenate two unrelated iteration series.
Its banners are at lines 8 and 484; each run ends in `All done: wannier90 exiting`.

## `derived/` — constructed, because nothing upstream has this shape

| File | Bytes | Banners | Resuming | DIS | CONV |
|---|---:|---:|---:|---:|---:|
| `bas_bannerless.wout` | 45889 | **0** | 1 | 6 | 19 |

EPW can append to an existing `.wout` without re-emitting the banner, producing a
file that begins mid-run. No generated or shipped `.wout` has that shape, so it is
cut from the second run of `bas_multirun.wout`, starting after its banner block:

```sh
sed -n '600,1266p' epw/bas_multirun.wout > derived/bas_bannerless.wout
```

Content is unmodified; only the cut points are ours. It parses to the same values
as the final run of `bas_multirun.wout` (DIS 3, spread 16, 8 WF), which is the
check that the banner-less path works.

## `wannier90/` — standalone wannier90 (NOT EPW) — **GPL, see NOTICE**

The only fixtures not generated locally. Taken verbatim from the Wannier90 3.1.0
test-suite (`$W90/test-suite/tests/testw90_*/benchmark.out.default.inp=*`).

| File | Bytes | Release | Banners | DIS | CONV | Upstream test |
|---|---:|---|---:|---:|---:|---|
| `h3s_standalone.wout` | 26370 | 3.1.0 | 1 | 13 | 6 | `testw90_disentanglement_sawfs` |
| `abort_nnkpt4.wout` | 133 | – | **0** | 0 | 0 | `testw90_nnkpt4` |
| `abort_nnkpt5.wout` | 145 | – | **0** | 0 | 0 | `testw90_nnkpt5` |

`h3s_standalone.wout` covers the standalone (non-EPW) shape: no `Resuming` line at
all. Note it puts `Welcome to the Maximally-Localized` on **line 7**, while
EPW-written `.wout` files have it on **line 8** — the w90 testcode benchmarks strip
a leading blank line. Both spellings are represented on purpose.

The two `abort_*` files are 3-line failure output — wannier90 bails before printing
its banner. They assert the parser survives truncated garbage rather than crashing.

## `own/` — locally generated, cases nothing upstream has

| File | Bytes | Why it is here |
|---|---:|---|
| `ph_qsplit.out` | 26584 | ph.x split over q-points. No QE test-suite input uses `start_q`/`last_q`. |
| `ph_q40.out` | 30272 | single-q ph.x run reading its irrep count from the grid table |
| `pt_no_conv.wout` | 157247 | disentanglement ending **not converged** (503 DIS, 54 CONV) |
