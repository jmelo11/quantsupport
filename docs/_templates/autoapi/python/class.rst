{% if obj.display %}
{% if is_own_page %}
{{ obj.short_name }}
{{ "=" * obj.short_name | length }}

{% endif %}
.. py:{{ obj.type }}:: {{ obj.id if is_own_page else obj.short_name }}({{ obj|constructor_signature }})

{% if obj.bases %}
   Bases: {% for base in obj.bases %}{{ base|link_objs }}{% if not loop.last %}, {% endif %}{% endfor %}

{% endif %}
{% if obj.docstring %}
   {{ obj.docstring|indent(3) }}

{% endif %}
   `Source declaration <{{ obj|declaration_source }}>`_

{% set visible = obj.children|selectattr("display")|list %}
{% for title, kind in [("Methods", "method"), ("Properties", "property"), ("Class Attributes", "attribute")] %}
{% set members = visible|selectattr("type", "equalto", kind)|list %}
{% if members %}
   .. rubric:: {{ title }}

{% for child in members %}
   {{ child.render()|indent(3) }}

{% endfor %}
{% endif %}
{% endfor %}
{% endif %}