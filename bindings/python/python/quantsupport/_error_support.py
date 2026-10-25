"""Error implementation executed in the native module's namespace at initialization.

Keeping the exception types in the native module gives direct extension imports
and the public Python packages the same exception identities.
"""

import functools as _functools
import inspect as _inspect
import types as _types


def _initialize(QuantSupportError):
    """Initialize error support using the native exception base."""
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


    class QuantSupportIOError(QuantSupportError, OSError):
        """An I/O operation failed; also catchable as OSError."""
        __module__ = "quantsupport"


    class QuantSupportNotImplementedError(QuantSupportError, NotImplementedError):
        """The requested operation is not implemented; also a NotImplementedError."""
        __module__ = "quantsupport"


    def _make_error(message, code="unexpected", operation=None, path=None,
                    expected=None, actual=None, contexts=(), details=None, native_type=None):
        if code == "invalid_type":
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
            contexts = error.contexts
            if error.operation and error.operation != operation:
                contexts = (error.operation, *contexts)
            return _make_error(error.message, error.code, operation or error.operation,
                               path or error.path, expected or error.expected, actual or error.actual,
                               contexts, error.details, error.native_type)
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
                return function(*args, **kwargs)
            except QuantSupportError as error:
                if error.operation == operation:
                    raise
                raise _with_context(error, operation) from error
            except StopIteration:
                raise
            except Exception as error:
                raise _with_context(error, operation) from error
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
            return allocator(cls, *args, **kwargs)
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

    for cls in (QuantSupportTypeError, QuantSupportValueError, QuantSupportStateError,
                QuantSupportIOError, QuantSupportNotImplementedError):
        cls.__qualname__ = cls.__name__
    return {
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
