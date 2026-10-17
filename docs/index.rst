QuantSupport Documentation
==========================

QuantSupport exposes financial contracts, market construction, valuation, and
risk through Python and a Windows Excel add-in. Choose an interface below.
Each user guide explains its own financial and operational conventions and
contains complete workflows; neither requires the Book as a prerequisite.

.. toctree::
   :maxdepth: 2

   Python <python/index>
   Excel <excel/index>
   coverage

Start with the :doc:`Python user guide <python/guide/index>` or
:doc:`Excel user guide <excel/guide/index>`. The
:doc:`computational studies <python/studies/index>` investigate curve
construction, valuation, simulation, and risk using the official notebooks.

The :doc:`Python reference <autoapi/quantsupport/index>` and
:doc:`Excel reference <excel/functions/index>` describe exact callable
interfaces: signatures, parameter meanings and units, defaults, result schemas,
and errors. They are generated from the packaged Python stubs and the Excel
registration metadata. The :doc:`coverage` page distinguishes direct wrappers,
configuration-based workflows, and remaining native-only components.

For an additional native-library learning path, see the optional
`QuantSupport Book <https://jmelo11.github.io/quantsupport/>`_ and
`Rust reference <https://docs.rs/quantsupport>`_. The local guides and reference
remain usable in a standalone Sphinx build.

.. rubric:: Indexes

* :ref:`genindex`
* :ref:`py-modindex`
* :ref:`excel-functionindex`
* :ref:`search`