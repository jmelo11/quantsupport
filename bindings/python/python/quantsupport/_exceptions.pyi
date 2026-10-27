"""Structured exceptions with categories shared with native QSError."""
from typing import Any, Mapping, Sequence

class QuantSupportError(Exception):
    """Base for QuantSupport failures, preserving the native category and cause.

    Parameters
    ----------
    message : str
        Human-readable description, separate from category and location.
    code : str, optional
        Stable native QSError code; defaults to unexpected.
    operation : str or None, optional
        Public constructor or method that failed.
    path : str or None, optional
        Input field and indices relative to the operation.
    expected : str or None, optional
        Required type or input shape, when applicable.
    actual : str or None, optional
        Supplied type or input shape, when applicable.
    contexts : sequence of str, optional
        Native and Python operation context, outermost first.
    details : mapping or None, optional
        Additional structured information such as file, line, and column.
    native_type : str or None, optional
        Original native QSError variant name, when available.
    """
    message: str
    code: str
    operation: str | None
    path: str | None
    expected: str | None
    actual: str | None
    contexts: tuple[str, ...]
    details: dict[str, Any]
    native_type: str | None
    def __init__(self, message: str, code: str = ..., operation: str | None = ...,
                 path: str | None = ..., expected: str | None = ..., actual: str | None = ...,
                 contexts: Sequence[str] = ..., details: Mapping[str, Any] | None = ...,
                 native_type: str | None = ...) -> None: ...
    def to_dict(self) -> dict[str, Any]:
        """Return error fields and the explicit cause chain as dictionaries."""
        ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class QuantSupportTypeError(QuantSupportError, TypeError):
    """Incompatible input type; catch as QuantSupportError or TypeError."""
    ...

class QuantSupportValueError(QuantSupportError, ValueError):
    """Invalid input value; catch as QuantSupportError or ValueError."""
    ...

class QuantSupportStateError(QuantSupportError, RuntimeError):
    """Invalid operation state; catch as QuantSupportError or RuntimeError."""
    ...

class QuantSupportIOError(QuantSupportError, OSError):
    """I/O failure; catch as QuantSupportError or OSError."""
    ...

class QuantSupportNotImplementedError(QuantSupportError, NotImplementedError):
    """Unsupported operation; catch as QuantSupportError or NotImplementedError."""
    ...

class UnresolvedProductErr(QuantSupportStateError):
    """Product construction requires an explicit reference date or market observation."""
    ...
