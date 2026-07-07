# pr-reviewer-test-mono

Private Stout fixture: a **monorepo-shaped firmware repo** for end-to-end
testing of the Stout PR Reviewer's monorepo capabilities on real hardware
(an nRF5340-DK on a lager box). It replicates the *structure* of a large
embedded monorepo — multiple products, shared code, umbrella CI with dynamic
artifact names, directory-module lager tests, a shared test-support package —
without containing anything proprietary.

## Layout

```
common/blink-config/   shared crate: blink-period policy (both products use it)
blinka/                product A — nRF5340 blink @ 700 ms (7x base)
  .cargo/config.toml   pins the embedded target; build from blinka/, not root
  src/main.rs          prints `blink <N> period=<MS>ms` over RTT ch 0
  tests/lager/blink-period/{main.py,limits.csv}   directory-module HIL test
blinkb/                product B — same shape @ 400 ms (4x base)
dtest/                 shared test-support package the module tests import
.github/workflows/
  ci-pipeline.yml      umbrella PR pipeline: paths-filter -> per-product builds
  blinka-pr.yml        workflow_call child; uploads blinka-v0.0.0.<run> (+ -manual decoy)
  blinkb-pr.yml        same for product B
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
