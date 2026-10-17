Python API
==========

Each class has its own declaration page with constructors, parameter details,
methods, properties, and class attributes. Domain packages group related types;
the package root re-exports the same class objects for compatibility. Each domain
is divided into focused subsections, such as Calendars, Yield Curves, Fixed
Income, and Collateral and Netting. See :doc:`../coverage` for the distinction
between direct wrappers, configuration-based access, and Rust-only components.

.. toctree::
   :maxdepth: 2

   ../autoapi/quantsupport/index

Conventions
-----------

Parameters show the accepted types in the declaration. A default displayed as
``...`` comes from the native implementation; the parameter description states
its actual default or how that default is selected. Dates accept the documented
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

See the `Python binding README <https://github.com/jmelo11/quantsupport/blob/main/bindings/python/README.md>`_
for installation and build instructions, and the
`Python walkthrough <../../python/getting-started.html>`_ for a first valuation.