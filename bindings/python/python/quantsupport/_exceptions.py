"""Exceptions raised by QuantSupport operations."""

from .quantsupport import (
    UnresolvedProductErr as UnresolvedProductErr,
    QuantSupportError, QuantSupportTypeError, QuantSupportValueError,
    QuantSupportStateError, QuantSupportIOError, QuantSupportNotImplementedError,
)

__all__ = ["QuantSupportError", "QuantSupportTypeError", "QuantSupportValueError",
           "QuantSupportStateError", "QuantSupportIOError", "QuantSupportNotImplementedError"]

__all__ += ["UnresolvedProductErr"]
