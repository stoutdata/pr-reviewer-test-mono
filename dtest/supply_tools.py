"""Power-supply net helpers shared by this monorepo's lager tests.

Thin wrappers over the lager `Net` power-supply API. The lager imports live
INSIDE the functions so that importing `dtest` (or its pure-parsing modules
like `uart_tools` and `rtt_tools`) off-box never pulls in the on-box lager
package.
"""


def get_supply(name):
    """The power-supply net called `name`, or None if the box has no such
    net (`Net.get` returns None for unknown names)."""
    from lager import Net, NetType
    return Net.get(name, type=NetType.PowerSupply)


def set_mv(net, mv):
    """Command `net` to `mv` millivolts.

    NOTE: the box's set_voltage path is asynchronous at the instrument end —
    the output typically takes ~0.5-0.7 s to settle at the new level — so
    callers must wait (the tests' --settle-s knob) before trusting readings
    taken after a set.
    """
    net.set_voltage(mv / 1000.0)


def enable(net):
    """Enable the supply output."""
    net.enable()


def disable(net):
    """Disable the supply output."""
    net.disable()
