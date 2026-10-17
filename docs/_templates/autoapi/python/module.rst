{% if obj.display %}
{% set domain = obj.name.rsplit('.', 1)[-1] %}
{% set sections = obj.name|python_sections %}
{% if sections %}
{{ domain|domain_title }}
{{ "=" * (domain|domain_title)|length }}

.. py:module:: {{ obj.name }}

{{ obj.docstring }}

.. toctree::
   :maxdepth: 2

{% for title, path in sections %}
   {{ title }} <{{ path }}>
{% endfor %}

{% else %}
Python Domains
==============

.. py:module:: {{ obj.name }}

.. toctree::
   :maxdepth: 2

{% for domain in ['time', 'market', 'instruments', 'pricing', 'risk'] %}
   {{ domain|domain_title }} </autoapi/quantsupport/{{ domain }}/index>
{% endfor %}

{% endif %}
{% endif %}