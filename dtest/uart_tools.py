"""UART log parsing helpers shared by the voltage-monitor products' tests.

Pure text parsing, no lager imports — importable off-box, same as rtt_tools.
"""

import re

# The firmware prints one line per sample: `vmon <N> mv=<MV>`.
_VMON_RE = re.compile(r"vmon \d+ mv=(\d+)")

def extract_mv(text: str) -> list:
    """All reported millivolt values (mV) present in a captured UART text
    buffer, in order. Tolerant of interleaved noise and partial lines:
    anything not matching the full `vmon <N> mv=<MV>` shape is ignored."""
    return [int(m.group(1)) for m in _VMON_RE.finditer(text)]
