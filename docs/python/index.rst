Python
======

`Download Python packages <https://jmelo11.github.io/quantsupport/python/downloads.html>`_
as folder ZIPs or wheels. Choose the package for your operating system,
architecture and Python version. Folder ZIPs contain only QuantSupport; install
dependencies separately with ``python -m pip install pandas``.

The Python package provides native QuantSupport instruments, market
construction, valuation and risk calculations. Define the contract and position,
configure the market, then request values, cashflows or sensitivities from a
pricing context. The user guide develops this workflow with complete examples;
the studies apply it to larger quantitative finance calculations.

.. toctree::
   :maxdepth: 2

   Download Python packages <https://jmelo11.github.io/quantsupport/python/downloads.html>
   guide/index
   studies/index
   API reference <../autoapi/quantsupport/index>

Each class has its own declaration page with constructors, parameter details,
methods, properties, and class attributes. Domain packages group related types;
the package root re-exports the same class objects for compatibility. Each domain
is divided into focused subsections, such as Calendars, Yield Curves, Fixed
Income, and Collateral and Netting.

Conventions
-----------

Signatures and parameter descriptions document accepted runtime input forms.
The reference is generated from the packaged stubs and their docstrings.
A default displayed as ``...`` comes from the native implementation; its
parameter description states the actual default or how it is selected.
Dates accept the documented
``Date``, ISO text, or ``datetime.date`` inputs. Rates and spreads are decimal
annual rates unless a parameter specifies other units.

Properties without setters are read-only. Context input properties with setters
invalidate the dependent constructed market when reassigned. Method calls such
as ``evaluate()`` perform operations rather than exposing attributes.

QuantSupport failures raise :py:class:`~quantsupport.QuantSupportError` or one of
its subclasses. Native and Python interfaces share stable error codes and native
variant names. Type, value, state, I/O, and unsupported-operation subclasses also
inherit the corresponding Python built-in exception, preserving existing catches.

Exceptions
----------

.. py:exception:: quantsupport.QuantSupportError

   Shared structured failure for construction, input conversion, market-data
   resolution, and evaluation. Inspect ``code``, ``native_type``, ``message``,
   ``operation``, ``path``, ``expected``, ``actual``, ``contexts``, and ``details``.
   Fields that do not apply are ``None``. ``__cause__`` preserves the original
   exception chain; ``to_dict()`` exports the same fields and nested causes.

The Python subclasses are :py:class:`~quantsupport.QuantSupportTypeError`,
:py:class:`~quantsupport.QuantSupportValueError`,
:py:class:`~quantsupport.QuantSupportStateError`,
:py:class:`~quantsupport.QuantSupportIOError`, and
:py:class:`~quantsupport.QuantSupportNotImplementedError`.

.. code-block:: python

   try:
       quotes = qs.QuoteStore.from_dict(market_inputs)
   except qs.QuantSupportError as error:
       print(error.code, error.operation, error.path)
       diagnostics = error.to_dict()

For example, an invalid quote level can report ``code="deserialization"``,
``native_type="DeserializationErr"``, and ``path="quotes[1].mid"``. A missing
quote instead reports ``code="not_found"`` and ``native_type="NotFoundErr"``.
These come from native ``QSError`` categories, rather than message matching.
File failures include the file path in ``details``; JSON failures also retain
line and column when the parser supplies them.

Python protocols retain normal behavior for attribute lookup, missing mapping
keys, iteration termination, and indexing: ``AttributeError``, ``KeyError``,
``StopIteration``, and ``IndexError`` continue to work with Python's containers
and inspection functions.

Installation, a first valuation, protocols, and reproducibility are explained
locally in :doc:`guide/first-valuation`. The
`binding README <https://github.com/jmelo11/quantsupport/blob/main/bindings/python/README.md>`_
also contains source-build instructions and development checks.
