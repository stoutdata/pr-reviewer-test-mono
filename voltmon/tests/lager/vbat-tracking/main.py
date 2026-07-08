"""voltmon vbat-tracking — directory-module HIL test (lager python convention).

Sweeps the DUT's monitored rail across the points in the limits table and
asserts the firmware's reported millivolts (the `vmon <N> mv=<MV>` lines on
UARTE0 TX / P1.05) track the commanded supply voltage. Assumes a PRE-FLASHED
DUT: flashing is the dispatcher's job (the Stout PR Reviewer's staging
preamble, or a manual `lager debug ... flash` before a hand run).

Wiring (see the repo README): supply HI through 1 kOhm to P0.04/AIN0, LO to
GND; USB-UART adapter RX on P1.05, common GND.

Run via the PR Reviewer, or manually from the repo root:

    lager python voltmon/tests/lager/vbat-tracking --box <BOX> -- -v

Exit codes (firmware-mono convention):
    0  pass
    1  device failure (reported millivolts out of tolerance)
    2  infra / precondition failure (missing inputs, nets, or UART traffic)
"""

import csv
import pathlib
import sys
import time

import click
from lager import Net, NetType

from dtest import supply_tools
from dtest.uart_tools import extract_mv


def read_limits(path: pathlib.Path):
    with path.open(newline="") as f:
        row = next(csv.DictReader(f))
    points = [int(p) for p in row["points_mv"].split(";")]
    return points, int(row["tol_mv"]), int(row["min_hits"])


@click.command()
@click.option("--supply-net", default="vbat", show_default=True,
              help="Power-supply net driving the monitored rail.")
@click.option("--uart-net", default="console", show_default=True,
              help="UART net wired to the DUT's P1.05 TX pin.")
@click.option("--settle-s", default=1.0, show_default=True,
              help="Wait after each voltage set (set_voltage settles "
                   "asynchronously, ~0.5-0.7 s).")
@click.option("--capture-s", default=3.0, show_default=True,
              help="How long to capture UART output at each point.")
@click.option("--limits", default="limits.csv", show_default=True,
              help="Limits table next to this test.")
@click.option("--elf", default="voltmon", show_default=True,
              help="Build ELF staged next to the test (presence proves the "
                   "dispatcher delivered the build outputs).")
@click.option("-v", "verbose", is_flag=True, help="Print each raw UART capture.")
def main(supply_net, uart_net, settle_s, capture_s, limits, elf, verbose):
    here = pathlib.Path(".")

    limits_path = here / limits
    if not limits_path.exists():
        click.echo(f"INFRA: limits table {limits} not found in {here.resolve()}")
        sys.exit(2)
    points, tol_mv, min_hits = read_limits(limits_path)

    if not (here / elf).exists():
        click.echo(
            f"INFRA: build ELF '{elf}' not staged next to the test — the "
            "dispatcher did not deliver the CI build outputs"
        )
        sys.exit(2)

    try:
        supply = supply_tools.get_supply(supply_net)
    except Exception as exc:
        click.echo(f"INFRA: could not get power-supply net '{supply_net}': {exc}")
        sys.exit(2)
    if supply is None:
        click.echo(f"INFRA: power-supply net '{supply_net}' not found on this box")
        sys.exit(2)

    try:
        uart = Net.get(uart_net, type=NetType.UART)
    except Exception as exc:
        click.echo(f"INFRA: could not get UART net '{uart_net}': {exc}")
        sys.exit(2)
    if uart is None:
        click.echo(f"INFRA: UART net '{uart_net}' not found on this box")
        sys.exit(2)

    ser = None
    results = []  # (point_mv, samples_seen, hits)
    try:
        ser = uart.connect(baudrate=115200, timeout=0.5)
        supply_tools.enable(supply)
        for point in points:
            supply_tools.set_mv(supply, point)
            time.sleep(settle_s)
            ser.reset_input_buffer()  # drop lines from the previous point
            deadline = time.time() + capture_s
            buf = b""
            while time.time() < deadline:
                chunk = ser.read(256)
                if chunk:
                    buf += chunk
            text = buf.decode("utf-8", errors="replace")
            if verbose:
                click.echo(f"---- UART capture @ {point}mV ----")
                click.echo(text)
                click.echo("---- end capture ----")
            samples = extract_mv(text)
            hits = sum(1 for mv in samples if abs(mv - point) <= tol_mv)
            results.append((point, len(samples), hits))
    except Exception as exc:
        click.echo(f"INFRA: lager/UART failure mid-sweep: {exc}")
        sys.exit(2)
    finally:
        # ALWAYS park the rail: 0 V and output off, whatever happened above.
        try:
            supply_tools.set_mv(supply, 0)
            supply_tools.disable(supply)
        except Exception:
            pass
        if ser is not None:
            try:
                ser.close()
            except Exception:
                pass

    if sum(n for _, n, _ in results) == 0:
        click.echo("INFRA: no vmon lines captured on UART (target not running? wiring?)")
        sys.exit(2)

    failed = False
    click.echo(f"{'point_mv':>9} {'samples':>8} {'hits':>5} {'min':>4}  result")
    for point, n, hits in results:
        ok = hits >= min_hits
        failed = failed or not ok
        click.echo(f"{point:>9} {n:>8} {hits:>5} {min_hits:>4}  {'ok' if ok else 'FAIL'}")
    if not failed:
        click.echo("PASS")
        sys.exit(0)
    click.echo(
        f"FAIL: at least one point missed {min_hits} samples within {tol_mv}mV "
        "of the commanded level"
    )
    sys.exit(1)


if __name__ == "__main__":
    main()
