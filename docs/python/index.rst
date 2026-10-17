Python
======

The Python interface separates contractual instruments from positions and
market inputs from constructed pricing objects. Begin with a complete
valuation, then study construction, model choice, and risk interpretation.

.. toctree::
   :maxdepth: 2

   guide/index
   studies/index
   API reference <../autoapi/quantsupport/index>

Each class has its own declaration page with constructors, parameter details,
methods, properties, and class attributes. Domain packages group related types;
the package root re-exports the same class objects for compatibility. Each domain
is divided into focused subsections, such as Calendars, Yield Curves, Fixed
Income, and Collateral and Netting. See :doc:`../coverage` for the distinction
between direct wrappers, configuration-based access, and Rust-only components.

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

Native failures raise ``quantsupport.QuantSupportError``. Invalid argument types
or values can raise ``TypeError`` or ``ValueError``; individual methods document
additional constraints and context lifetime requirements.

Exceptions
----------

.. py:exception:: quantsupport.QuantSupportError

   Library exception raised when native construction, market-data resolution,
   or numerical evaluation fails. The message includes the underlying error.

Installation, a first valuation, protocols, and reproducibility are explained
locally in :doc:`guide/first-valuation`. The
`binding README <https://github.com/jmelo11/quantsupport/blob/main/bindings/python/README.md>`_
is an additional source-distribution entry point, not a prerequisite.