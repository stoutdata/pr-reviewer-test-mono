"""RTT log parsing helpers shared by every product's blink-period test."""

import re

# The firmware prints one line per LED toggle: `blink <N> period=<MS>ms`.
_BLINK_RE = re.compile(r"blink \d+ period=(\d+)ms")


def extract_periods(text: str) -> list:
    """All period values (ms) present in a captured RTT text buffer, in order."""
    return [int(m.group(1)) for m in _BLINK_RE.finditer(text)]
