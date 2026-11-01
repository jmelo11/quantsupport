"""Error implementation executed in the native module's namespace at initialization.

Keeping the exception types in the native module gives direct extension imports
and the public Python packages the same exception identities.
"""

import functools as _functools
import inspect as _inspect
import textwrap as _textwrap
import traceback as _traceback
import types as _types


def _initialize(QuantSupportError):
    """Initialize error support using the native exception base."""
    def _user_traceback(traceback, notebook=False):
        """Remove embedded boundary frames while retaining the caller's stack."""
        frames = []
        while traceback is not None:
            frame = traceback.tb_frame
            internal = (frame.f_globals is _initialize.__globals__
                        and frame.f_code.co_filename == "<string>")
            shell = notebook and frame.f_globals.get("__name__", "").startswith("IPython.")
            if not internal and not shell:
                frames.append(traceback)
            traceback = traceback.tb_next
        result = None
        for frame in reversed(frames):
            result = _types.TracebackType(result, frame.tb_frame, frame.tb_lasti, frame.tb_lineno)
        return result


    def _error_init(self, message, code="unexpected", operation=None, path=None,
                    expected=None, actual=None, contexts=(), details=None, native_type=None):
        Exception.__init__(self, str(message))
        self.message = str(message)
        self.code = code
        self.native_type = native_type
        self.operation = operation
        self.path = path
        self.expected = expected
        self.actual = actual
        self.contexts = tuple(contexts)
        self.details = dict(details or {})


    def _error_str(self):
        location = self.operation or ""
        if self.path:
            location = f"{location}.{self.path}" if location else self.path
        return f"[{self.code}] {location + ': ' if location else ''}{self.message}"


    def _error_repr(self):
        return (f"{type(self).__name__}(message={self.message!r}, code={self.code!r}, "
                f"operation={self.operation!r}, path={self.path!r})")


    def _error_panel(self, formatter, traceback_lines=()):
        """Connect the calling code and structured diagnostics in one panel."""
        theme = None
        if formatter is not None and hasattr(formatter, "_theme_name"):
            from IPython.utils.PyColorize import theme_table
            from pygments.token import Token
            theme = theme_table[formatter._theme_name]

        def paint(role, value):
            if theme is None:
                return value
            return theme.format([(getattr(Token, role), value)]).rstrip("\n")

        rail = paint("Topline", "│")
        lines = []
        if traceback_lines:
            lines.append(paint("Topline", "╭─ ")
                         + paint("NormalEm", "Traceback (most recent call last):"))
            lines.extend(rail + "  " + line for line in traceback_lines)
            lines.append(rail)
        heading = "├─ " if traceback_lines else "╭─ "
        lines.append(paint("Topline", heading) + paint("ExcName", type(self).__name__)
                     + " " + paint("NormalEm", f"[{self.code}]"))

        def row(value, label="", role="Text"):
            first_prefix = f"  {label:<9} " if label else "  "
            next_prefix = " " * len(first_prefix)
            for paragraph in str(value).split("\n"):
                for part in (_textwrap.wrap(paragraph, width=88 - len(first_prefix),
                                           break_long_words=False, break_on_hyphens=False) or [""]):
                    if label:
                        prefix = "  " + paint("VName", f"{label:<9}") + " "
                    else:
                        prefix = next_prefix
                    lines.append(rail + prefix + paint(role, part))
                    label = ""

        message = self.message
        if (self.code == "invalid_type" and self.expected is not None and self.actual is not None
                and message == f"Invalid type error: expected {self.expected}, got {self.actual}"):
            message = "Invalid input type."
        lines.append(rail)
        row(message)
        fields = [("Operation", self.operation, "NormalEm"), ("Input", self.path, "NormalEm"),
                  ("Expected", self.expected, "ValEm"), ("Received", self.actual, "ExcName")]
        if any(value is not None for _, value, _ in fields):
            lines.append(rail)
            for label, value, role in fields:
                if value is not None:
                    row(value, label, role)
        if self.contexts:
            lines.append(rail)
            for context in self.contexts:
                row(context, "Context")
        for note in getattr(self, "__notes__", ()):
            row(note, "Note")
        lines.append(paint("Topline", "╰" + "─" * 57))
        return lines


    def _error_render_traceback(self):
        """Show the caller's stack followed by its failure in one continuous panel."""
        try:
            from IPython import get_ipython
        except ImportError:
            shell = None
        else:
            shell = get_ipython()
        formatter = shell.InteractiveTB if shell is not None else None
        traceback_lines = []
        traceback = _user_traceback(self.__traceback__, notebook=True)
        if traceback is not None:
            if formatter is None:
                frames = _traceback.format_tb(traceback)
            elif hasattr(formatter, "get_records"):
                records = formatter.get_records(traceback, context=3, tb_offset=0)
                if (hasattr(formatter, "_theme_name")
                        and any(record._sd is None for record in records)):
                    # Notebook source lives in linecache rather than real files.
                    # IPython's file fallback cuts multiline statements; let its
                    # source parser recover the complete calls from the cache.
                    import stack_data
                    from IPython.core.tbtools import FrameInfo
                    from IPython.utils.PyColorize import theme_table
                    from pygments.formatters import Terminal256Formatter

                    options = stack_data.Options(before=1, after=1)
                    if formatter.has_colors:
                        style = theme_table[formatter._theme_name].as_pygments_style()
                        options.pygments_formatter = Terminal256Formatter(style=style)
                    records = [FrameInfo._from_stack_data_FrameInfo(record) for record in
                               stack_data.FrameInfo.stack_data(traceback, options=options)]
                frames = [formatter.format_record(record) for record in records]
            else:
                frames = formatter._format_list(_traceback.extract_tb(traceback))
            traceback_lines = "".join(frames).strip("\n").splitlines()
        return _error_panel(self, formatter, traceback_lines)


    def _error_to_dict(self):
        def serialize(error, seen):
            if id(error) in seen:
                return {"type": type(error).__name__, "message": "cyclic exception cause"}
            seen.add(id(error))
            if not isinstance(error, QuantSupportError):
                return {"type": type(error).__name__, "message": str(error)}
            return {
                "code": error.code, "native_type": error.native_type,
                "message": error.message, "operation": error.operation,
                "path": error.path, "expected": error.expected, "actual": error.actual,
                "contexts": list(error.contexts), "details": dict(error.details),
                "cause": serialize(error.__cause__, seen) if error.__cause__ is not None else None,
            }
        return serialize(self, set())


    QuantSupportError.__init__ = _error_init
    QuantSupportError.__str__ = _error_str
    QuantSupportError.__repr__ = _error_repr
    QuantSupportError._render_traceback_ = _error_render_traceback
    QuantSupportError.to_dict = _error_to_dict


    class QuantSupportTypeError(QuantSupportError, TypeError):
        """An input has an incompatible type; also catchable as TypeError."""
        __module__ = "quantsupport"


    class QuantSupportValueError(QuantSupportError, ValueError):
        """An input has an invalid value; also catchable as ValueError."""
        __module__ = "quantsupport"


    class QuantSupportStateError(QuantSupportError, RuntimeError):
        """An operation cannot run in the current state; also a RuntimeError."""
        __module__ = "quantsupport"


    class UnresolvedProductErr(QuantSupportStateError):
        """Construction requires an explicit reference date or market observation."""
        __module__ = "quantsupport"


    class QuantSupportIOError(QuantSupportError, OSError):
        """An I/O operation failed; also catchable as OSError."""
        __module__ = "quantsupport"


    class QuantSupportNotImplementedError(QuantSupportError, NotImplementedError):
        """The requested operation is not implemented; also a NotImplementedError."""
        __module__ = "quantsupport"


    def _make_error(message, code="unexpected", operation=None, path=None,
                    expected=None, actual=None, contexts=(), details=None, native_type=None):
        if code == "unresolved_product":
            cls = UnresolvedProductErr
        elif code == "invalid_type":
            cls = QuantSupportTypeError
        elif code in ("invalid_value", "missing_value", "date_parsing", "period_parsing",
                      "parsing", "quote_parsing", "deserialization", "schedule"):
            cls = QuantSupportValueError
        elif code == "io":
            cls = QuantSupportIOError
        elif code == "not_implemented":
            cls = QuantSupportNotImplementedError
        elif code in ("evaluation", "node", "tape", "node_not_indexed", "dual_forward"):
            cls = QuantSupportStateError
        else:
            cls = QuantSupportError
        return cls(message, code, operation, path, expected, actual, contexts, details, native_type)


    def _with_context(error, operation=None, path=None, expected=None, actual=None):
        if isinstance(error, QuantSupportError):
            if operation is not None and operation != error.operation:
                if error.operation:
                    error.contexts = (error.operation, *error.contexts)
                error.operation = operation
            for name, value in (("path", path), ("expected", expected), ("actual", actual)):
                if value is not None:
                    setattr(error, name, value)
            return error
        if isinstance(error, NotImplementedError):
            code, native_type = "not_implemented", "NotImplementedErr"
        elif isinstance(error, TypeError):
            code, native_type = "invalid_type", "InvalidTypeErr"
        elif isinstance(error, ValueError):
            code, native_type = "invalid_value", "InvalidValueErr"
        elif isinstance(error, OSError):
            code, native_type = "io", "IoErr"
        elif isinstance(error, (LookupError, AttributeError)):
            code, native_type = "not_found", "NotFoundErr"
        elif isinstance(error, RuntimeError):
            code, native_type = "evaluation", "EvaluationErr"
        else:
            code, native_type = "unexpected", "UnexpectedErr"
        return _make_error(str(error), code, operation, path, expected, actual,
                           details={"python_type": type(error).__name__}, native_type=native_type)


    def _guard(function, operation):
        if getattr(function, "_quantsupport_guard", False):
            return function
        @_functools.wraps(function)
        def guarded(*args, **kwargs):
            try:
                try:
                    return function(*args, **kwargs)
                except QuantSupportError as error:
                    _with_context(error, operation)
                    raise
                except StopIteration:
                    raise
                except Exception as error:
                    error.__traceback__ = _user_traceback(error.__traceback__)
                    raise _with_context(error, operation) from error
            except QuantSupportError as error:
                error.__traceback__ = _user_traceback(error.__traceback__)
                raise
        guarded._quantsupport_guard = True
        return guarded


    def _guard_classmethod(descriptor, operation):
        def guarded(cls, *args, **kwargs):
            return descriptor.__get__(None, cls)(*args, **kwargs)
        # The descriptor's text signature includes cls; preserve it for introspection.
        guarded.__signature__ = _inspect.signature(descriptor)
        guarded.__doc__ = descriptor.__doc__
        return classmethod(_guard(guarded, operation))


    class _ConstructorSignature:
        """Preserve native signatures without hiding Python subclass constructors."""
        def __init__(self, owner, signature):
            self.owner = owner
            self.signature = signature

        def __get__(self, instance, owner):
            for base in owner.__mro__:
                if base is self.owner:
                    return self.signature
                if "__new__" in vars(base) or "__init__" in vars(base):
                    return None
            return None


    def _constructor_signature(owner, signature):
        return _ConstructorSignature(owner, signature)


    def _constructor(allocator, original):
        @_functools.wraps(original)
        def construct(cls, *args, **kwargs):
            try:
                return allocator(cls, *args, **kwargs)
            except QuantSupportError as error:
                error.__traceback__ = _user_traceback(error.__traceback__)
                raise
        return construct


    def _install_error_boundaries(namespace):
        """Guard public native methods without replacing native allocation slots."""
        for cls in tuple(namespace.values()):
            if not isinstance(cls, type) or cls.__module__ != "quantsupport" or issubclass(cls, Exception):
                continue
            for name, descriptor in tuple(vars(cls).items()):
                if name.startswith("_") and name not in ("__init__", "__enter__", "__exit__"):
                    continue
                operation = f"{cls.__name__}.{name}"
                if isinstance(descriptor, staticmethod):
                    setattr(cls, name, staticmethod(_guard(descriptor.__func__, operation)))
                elif isinstance(descriptor, classmethod):
                    setattr(cls, name, classmethod(_guard(descriptor.__func__, operation)))
                elif isinstance(descriptor, _types.ClassMethodDescriptorType):
                    setattr(cls, name, _guard_classmethod(descriptor, operation))
                elif _inspect.ismethoddescriptor(descriptor):
                    setattr(cls, name, _guard(descriptor, operation))

    for cls in (UnresolvedProductErr, QuantSupportTypeError, QuantSupportValueError, QuantSupportStateError,
                QuantSupportIOError, QuantSupportNotImplementedError):
        cls.__qualname__ = cls.__name__
    return {
        "UnresolvedProductErr": UnresolvedProductErr,
        "QuantSupportTypeError": QuantSupportTypeError,
        "QuantSupportValueError": QuantSupportValueError,
        "QuantSupportStateError": QuantSupportStateError,
        "QuantSupportIOError": QuantSupportIOError,
        "QuantSupportNotImplementedError": QuantSupportNotImplementedError,
        "_make_error": _make_error,
        "_with_context": _with_context,
        "_constructor_signature": _constructor_signature,
        "_constructor": _constructor,
        "_install_error_boundaries": _install_error_boundaries,
    }
