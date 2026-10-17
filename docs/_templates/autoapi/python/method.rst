{% if obj.display %}
.. py:method:: {{ obj.id if is_own_page else obj.short_name }}({{ obj|method_signature }}){% if obj.return_annotation is not none %} -> {{ obj.return_annotation }}{% endif %}
   {% for (args, return_annotation) in obj.overloads %}

               {%+ if is_own_page %}{{ obj.id }}{% else %}{{ obj.short_name }}{% endif %}({{ args|public_signature }}){% if return_annotation is not none %} -> {{ return_annotation }}{% endif %}
   {% endfor %}
   {% for property in obj.properties %}

   :{{ property }}:
   {% endfor %}

{% if obj.docstring %}
   {{ obj.docstring|indent(3) }}

{% endif %}
{% endif %}