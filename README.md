# pr-reviewer-test-mono

Private Stout fixture: a **monorepo-shaped firmware repo** for end-to-end
testing of the Stout PR Reviewer's monorepo capabilities on real hardware
(an nRF5340-DK on a lager box). It replicates the *structure* of a large
embedded monorepo — multiple products, shared code, umbrella CI with dynamic
artifact names, directory-module lager tests, a shared test-support package —
without containing anything proprietary.

## Layout

```
common/blink-config/   shared crate: blink-period policy (both blink products use it)
common/monitor-config/ shared crate: SAADC-code -> millivolts policy (voltmon uses it)
blinka/                product A — nRF5340 blink @ 700 ms (7x base)
  .cargo/config.toml   pins the embedded target; build from blinka/, not root
  src/main.rs          prints `blink <N> period=<MS>ms` over RTT ch 0
  tests/lager/blink-period/{main.py,limits.csv}   directory-module HIL test
blinkb/                product B — same shape @ 400 ms (4x base)
voltmon/               product C — nRF5340 SAADC voltage monitor, one sample / 250 ms
  src/main.rs          prints `vmon <N> mv=<MV>` over RTT ch 0 AND UARTE0 TX
                       (P1.05, 115200 8N1); SCALE_TRIM_MV is the trim knob
  tests/lager/vbat-tracking/{main.py,limits.csv}  supply-sweep HIL test
dtest/                 shared test-support package the module tests import
.github/workflows/
  ci-pipeline.yml      umbrella PR pipeline: paths-filter -> per-product builds
  blinka-pr.yml        workflow_call child; uploads blinka-v0.0.0.<run> (+ -manual decoy)
  blinkb-pr.yml        same for product B
  voltmon-pr.yml       same for product C (voltmon-v0.0.0.<run> + -manual decoy)
```

Structural properties under test (each maps to a PR Reviewer config field):

- **Path-scoped reviewers** — one reviewer per product; `blinka/` PRs must not
  trigger the blinkb reviewer; `common/` PRs trigger both (`pathPrefixes`).
- **Shared umbrella workflow** — artifacts attach to `ci-pipeline.yml`'s run;
  reviewers disambiguate by artifact-name **pattern** (`^blinka-v[0-9.]+$`),
  which the `-manual` decoy artifact keeps honest (`firmwareArtifactPattern`).
- **Multi-file artifact** — raw `.bin` (flashed) + ELF (staged next to the
  test, which presence-checks it) (`firmwareArtifactPath`, `testArtifactFiles`).
- **Dispatcher-owned flashing** — tests assume a pre-flashed DUT; the
  reviewer's preamble flashes the raw binary at `0x0` with erase
  (`firmwareStageName`, `firmwareLoadAddress`, `firmwareFlashErase`,
  `debugNetName`).
- **Directory-module tests** — `main.py` + `limits.csv`, importing `dtest`,
  argv after `--`, exit codes 0 pass / 1 device-fail / 2 infra
  (`testMode`, `testIncludes`, `testArgs`).

## Building locally

```bash
cd blinka && cargo objcopy --release -- -O binary blinka.bin
```

Requires `cargo-binutils` (install from your home dir, not a product dir).
The repo-root `rust-toolchain.toml` provisions the target + llvm-tools.

## Running a test manually

Flash first (the tests do NOT flash), then run the module:

```bash
lager debug debug1 flash --bin blinka/blinka.bin,0x0 --box <BOX>
lager python blinka/tests/lager/blink-period --box <BOX> -- -v
```

(A manual `lager python` run does not stage `dtest`/the ELF the way the
reviewer does; pass `--add-file` equivalents if running by hand.)

### voltmon wiring (vbat-tracking)

The blink products need no bench wiring; voltmon does:

- Keithley HI -> **1 kOhm series** -> DK **P0.04 / AIN0**; Keithley LO -> DK GND.
  (The resistor protects the pin if the supply is misprogrammed.)
- USB-UART adapter RX -> DK **P1.05** (a free pin on header P4, chosen because
  no DK LED/button/VCOM route touches it); adapter GND -> DK GND.
- The test sweeps **0.5-2.5 V** (the limits table's points) — comfortably
  inside the firmware's 3.6 V SAADC full scale.

```bash
lager debug debug1 flash --bin voltmon/voltmon.bin,0x0 --box <BOX>
lager python voltmon/tests/lager/vbat-tracking --box <BOX> -- -v
```

## Scenario playbook (reviewer E2E)

1. **Happy path** — branch; in `blinka/src/main.rs` change `PERIOD_MULT` and
   update `blinka/tests/lager/blink-period/limits.csv` to match (the honest
   engineer change: firmware + limits together). Open a PR: only the blinka
   reviewer fires, parks awaiting the CI build, resumes, reuses the module
   test, and after approval flashes + passes at the new period.
2. **Routing isolation** — the same PR must create NO blinkb run.
3. **Shared-path fan-out** — a PR adding something harmless to
   `common/blink-config` (e.g. a new helper) fires BOTH reviewers; both can
   pass (periods unchanged).
4. **Failure/triage** — change `PERIOD_MULT` WITHOUT touching limits.csv:
   the reused test fails (exit 1) and the reviewer triages it as a firmware
   bug.

### voltmon fault-injection table

voltmon adds a five-outcome matrix on top of the blink scenarios. Each row is
a one-edit (or one-action) PR the reviewer must land on the right verdict:

| Outcome | Injection | Expected reviewer verdict |
| --- | --- | --- |
| 1 | Cadence tweak: `SAMPLE_PERIOD_MS` 250 -> e.g. 400 in `voltmon/src/main.rs` | Benign change — test still collects >= min_hits per point; passes. |
| 2 | `SCALE_TRIM_MV` 0 -> 150 in `voltmon/src/main.rs` | Firmware bug — every point reads ~150 mV high, outside the 60 mV tolerance (exit 1); triaged as a firmware defect. |
| 3 | Tolerance misuse in `main.py` (e.g. compare against `tol_mv` wrongly) or limits.csv `tol_mv` 60 -> 1 | Test bug — healthy firmware fails a too-tight check; triaged as a test defect, not firmware. |
| 4 | `SCALE_TRIM_MV` bug AND weaken main.py's final check to `hits >= 0` | Masked firmware bug — the test is gamed to always pass; the reviewer must flag the weakened assertion, not trust the green run. |
| 5 | Power off the UART hub port before the run | Infra failure — no UART traffic at all (exit 2); triaged as infrastructure, no code blame. |
