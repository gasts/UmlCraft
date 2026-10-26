from __future__ import annotations

import shop.orders as so
from typing import TYPE_CHECKING

import shop
from shop import Order, Line
from . import helpers

if TYPE_CHECKING:
    from shop.lines import Line as L


class View:
    # Re-exported by shop/__init__.py.
    order: Order
    line: Line
    # Through a module alias.
    aliased: so.Order
    # Through the package.
    qualified: shop.orders.Order
    # Under TYPE_CHECKING.
    checked: L
    # From a relative import of a module.
    helper: helpers.Helper
    # Unknown.
    missing: Missing
