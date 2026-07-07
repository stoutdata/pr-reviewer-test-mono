"""blinka blink-period — directory-module HIL test (lager python convention).

Measures the firmware's blink period by reading plain-text RTT and compares
it against the limits table committed next to this file. Assumes a
PRE-FLASHED DUT: flashing is the dispatcher's job (the Stout PR Reviewer's
staging preamble, or a manual `lager debug ... flash` before a hand run).

Run via the PR Reviewer, or manually from the repo root:

    lager python blinka/tests/lager/blink-period --box <BOX> -- -v

Exit codes (firmware-mono convention):
    0  pass
    1  device failure (blink period out of limits)
    2  infra / precondition failure (missing inputs, no RTT traffic)
"""

import csv
import pathlib
import sys
import time

import click
from lager import Net, NetType

from dtest.rtt_tools import extract_periods


def read_limits(path: pathlib.Path):
    with path.open(newline="") as f:
        row = next(csv.DictReader(f))
    return int(row["expected_period_ms"]), int(row["min_hits"])


@click.command()
@click.option("--debug-net", default="debug1", show_default=True,
              help="Debug net wired to this product's DUT.")
@click.option("--limits", default="limits.csv", show_default=True,
              help="Limits table next to this test.")
@click.option("--elf", default="blinka", show_default=True,
              help="Build ELF staged next to the test (presence proves the "
                   "dispatcher delivered the build outputs).")
@click.option("--read-seconds", default=8.0, show_default=True,
              help="How long to capture RTT after reset.")
@click.option("-v", "verbose", is_flag=True, help="Print the raw RTT capture.")
def main(debug_net, limits, elf, read_seconds, verbose):
    here = pathlib.Path(".")

    limits_path = here / limits
    if not limits_path.exists():
        click.echo(f"INFRA: limits table {limits} not found in {here.resolve()}")
        sys.exit(2)
    expected, min_hits = read_limits(limits_path)

    if not (here / elf).exists():
        click.echo(
            f"INFRA: build ELF '{elf}' not staged next to the test — the "
            "dispatcher did not deliver the CI build outputs"
        )
        sys.exit(2)

    dbg = Net.get(debug_net, type=NetType.Debug)
    buf = b""
    with dbg.session() as s:
        s.reset()
        with s.rtt(channel=0) as rtt:
            deadline = time.time() + read_seconds
            while time.time() < deadline:
                chunk = rtt.read_some(timeout=0.5)
                if chunk:
                    buf += chunk

    out = buf.decode("utf-8", errors="replace")
    if verbose:
        click.echo("---- RTT channel 0 ----")
        click.echo(out)
        click.echo("---- end RTT ----")

    periods = extract_periods(out)
    if not periods:
        click.echo("INFRA: no blink lines captured on RTT (target not running?)")
        sys.exit(2)

    hits = sum(1 for p in periods if p == expected)
    click.echo(
        f"captured {len(periods)} blink lines; period={expected}ms hits={hits} "
        f"(min {min_hits}); distinct periods seen: {sorted(set(periods))}"
    )
    if hits >= min_hits:
        click.echo("PASS")
        sys.exit(0)
    click.echo(f"FAIL: expected period={expected}ms was not observed enough")
    sys.exit(1)


if __name__ == "__main__":
    main()
