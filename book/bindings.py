"""Generate binding reference chapters through mdBook's preprocessor protocol."""

import ast
from collections import defaultdict
from copy import deepcopy
from html import escape
import json
from pathlib import Path
import re
import subprocess
import sys
import textwrap

from docstring_parser import parse


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = Path("bindings/python/python/quantsupport")
DOMAINS = ("time", "market", "instruments", "pricing", "risk")
ALIASES = {
    "_DateLike": "Date | str | datetime.date",
    "_PeriodLike": "Period | str",
    "_CurrencyLike": "Currency | str",
    "_IndexLike": "MarketIndex | str | Mapping[str, Any]",
    "PythonDate": "datetime.date",
}


def markdown(text):
    """Convert inline roles and literal blocks used by the typed docstrings."""
    text = re.sub(r":(?:class|meth|func|attr|mod):`([^`]+)`", r"`\1`", text)
    text = re.sub(r"``([^`\n]+)``", r"`\1`", text)
    lines = text.splitlines()
    result = []
    position = 0
    while position < len(lines):
        line = lines[position]
        if line.strip().startswith(".. code-block::"):
            language = line.split("::", 1)[1].strip() or "text"
            position += 1
            while position < len(lines) and not lines[position].strip():
                position += 1
            block = []
            while position < len(lines) and (not lines[position].strip() or lines[position].startswith(" ")):
                block.append(lines[position])
                position += 1
            result.extend([f"```{language}", textwrap.dedent("\n".join(block)).rstrip(), "```", ""])
            continue
        result.append(line)
        position += 1
    return "\n".join(result).strip()


def description(node):
    doc = parse(ast.get_docstring(node) or "")
    parts = [markdown(text) for text in (doc.short_description, doc.long_description) if text]
    previous = None
    headings = {"param": "Parameters", "returns": "Returns", "yields": "Yields",
                "raises": "Raises", "examples": "Examples", "example": "Examples"}
    for entry in doc.meta:
        kind = entry.args[0]
        if kind != previous:
            parts.append(f"**{headings.get(kind, kind.title())}**")
            previous = kind
        if kind == "param":
            annotation = f" ({entry.type_name})" if entry.type_name else ""
            parts.append(f"- `{entry.arg_name}`{annotation}: {markdown(entry.description or '')}")
        elif kind in ("examples", "example"):
            if entry.description:
                parts.append(markdown(entry.description))
            if entry.snippet:
                parts.append(f"```python\n{entry.snippet}\n```")
        else:
            annotation = getattr(entry, "type_name", None)
            parts.append((f"`{annotation}`: " if annotation else "") + markdown(entry.description or ""))
    return "\n\n".join(parts)


class PublicAnnotations(ast.NodeTransformer):
    def visit_Name(self, node):
        if node.id in ALIASES:
            return ast.parse(ALIASES[node.id], mode="eval").body
        return node


def signature(method, owner):
    method = PublicAnnotations().visit(deepcopy(method))
    arguments = method.args
    positional = arguments.posonlyargs or arguments.args
    if positional and positional[0].arg in ("self", "cls"):
        positional.pop(0)
    parameters = ast.unparse(arguments)
    if method.name in ("__new__", "__init__"):
        return f"{owner}({parameters})"
    returns = f" -> {ast.unparse(method.returns)}" if method.returns else ""
    return f"{method.name}({parameters}){returns}"


def python_reference(root, domain):
    source = PACKAGE / domain / "__init__.pyi"
    module = ast.parse((root / source).read_text(encoding="utf-8"))
    exports = next(ast.literal_eval(node.value) for node in module.body
                   if isinstance(node, ast.Assign)
                   and any(isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets))
    classes = {node.name: node for node in module.body if isinstance(node, ast.ClassDef)}
    lines = [f"# Python: {domain}", "", ast.get_docstring(module) or "", "",
             f"```python\nfrom quantsupport import {domain}\n```", "",
             "These classes are also available as `quantsupport.Name`. Defaults shown as `...` "
             "are supplied by the native constructor.", ""]
    lines.extend(f"- [{name}](#{name.lower()})" for name in exports)
    for name in exports:
        cls = classes[name]
        lines.extend(["", f"## {name} {{#{name.lower()}}}", "", description(cls), ""])
        if cls.bases:
            lines.extend(["Bases: " + ", ".join(f"`{ast.unparse(base)}`" for base in cls.bases) + ".", ""])
        methods = [node for node in cls.body if isinstance(node, ast.FunctionDef)]
        setters = {node.name for node in methods
                   if any(isinstance(decorator, ast.Attribute) and decorator.attr == "setter"
                          for decorator in node.decorator_list)}
        constructors = set()
        for method in methods:
            if method.name.startswith("_") and method.name not in ("__new__", "__init__"):
                continue
            if any(isinstance(decorator, ast.Attribute) and decorator.attr == "setter"
                   for decorator in method.decorator_list):
                continue
            decorators = {ast.unparse(decorator) for decorator in method.decorator_list}
            constructor = method.name in ("__new__", "__init__")
            if constructor:
                constructor_signature = signature(method, name)
                if constructor_signature in constructors and not ast.get_docstring(method):
                    continue
                constructors.add(constructor_signature)
            label = "Constructor" if constructor else f"{name}.{method.name}"
            lines.extend([f"### {label}", ""])
            if "property" in decorators:
                mode = "Read/write" if method.name in setters else "Read-only"
                annotation = ast.unparse(PublicAnnotations().visit(deepcopy(method.returns)))
                lines.extend([f"**{mode} property:** `{method.name}: {annotation}`", ""])
            else:
                if "classmethod" in decorators or "staticmethod" in decorators:
                    lines.extend(["**Class method.**" if "classmethod" in decorators else "**Static method.**", ""])
                lines.extend(["```python", signature(method, name), "```", ""])
            if ast.get_docstring(method):
                lines.extend([description(method), ""])
        attributes = [node for node in cls.body if isinstance(node, ast.AnnAssign)
                      and isinstance(node.target, ast.Name) and not node.target.id.startswith("_")]
        if attributes:
            lines.extend(["### Class Attributes", "", "```python"])
            lines.extend(ast.unparse(node) for node in attributes)
            lines.extend(["```", ""])
        lines.extend([f"[Typed source](https://github.com/jmelo11/quantsupport/blob/main/{source.as_posix()})", ""])
    return "\n".join(lines)


def excel_reference(category, records):
    lines = [f"# Excel: {category}", ""]
    for record in records:
        name = record["name"]
        arguments = record["arguments"]
        lines.extend([f"## {name} {{#{name.lower().replace('.', '-')}}}", "", "```text",
                      f"={name}({', '.join(argument['name'] for argument in arguments)})", "```", "",
                      escape(record["help"], quote=False), ""])
        if record["aliases"]:
            lines.extend(["Aliases: " + ", ".join(f"`{alias}`" for alias in record["aliases"]) + ".", ""])
        if arguments:
            lines.extend(["| Parameter | Meaning |", "| --- | --- |"])
            for argument in arguments:
                help_text = escape(argument["help"], quote=False).replace("|", "\\|").replace("\n", " ")
                lines.append(f"| `{argument['name']}` | {help_text} |")
            lines.append("")
        calculation = "Thread-safe scalar function." if record["threadsafe"] else "Excel main calculation thread."
        if record["volatile"]:
            calculation += " Volatile registration; unchanged inputs reuse cached objects."
        lines.extend([calculation, "", f"[Rust source](https://github.com/jmelo11/quantsupport/blob/main/{record['source']})", ""])
    return "\n".join(lines)


def chapter(name, path, content, parent):
    number = parent.get("number")
    return {"Chapter": {"name": name, "content": content, "path": path, "source_path": None,
                        "number": [*number, len(parent["sub_items"]) + 1] if number else None,
                        "sub_items": [], "parent_names": [*parent.get("parent_names", []), parent["name"]]}}


def preprocess(context, book):
    root = Path(context["root"])
    result = subprocess.run(["cargo", "run", "--quiet", "--package", "quantsupport-xll",
                             "--example", "function_catalog"], cwd=root, text=True, capture_output=True, check=True)
    records = json.loads(result.stdout)
    if not records:
        raise ValueError("The Excel function catalog is empty")
    groups = defaultdict(list)
    for record in records:
        groups[record["category"].removeprefix("QuantSupport - ")].append(record)
    found = set()

    def visit(sections):
        for section in sections:
            item = section.get("Chapter") if isinstance(section, dict) else None
            if item is None:
                continue
            path = item.get("path")
            if path == "python/api.md":
                found.add(path)
                item["sub_items"] = []
                for domain in DOMAINS:
                    item["sub_items"].append(chapter(domain.title(), f"python/reference/{domain}.md",
                                                       python_reference(root, domain), item))
            elif path == "excel/reference.md":
                found.add(path)
                item["sub_items"] = []
                item["content"] += f"\n\n{len(records)} functions, generated from the XLL registration attributes.\n\n"
                for category, functions in sorted(groups.items()):
                    slug = category.lower().replace(" ", "-")
                    item["content"] += f"- [{category}](reference/{slug}.md)\n"
                    item["sub_items"].append(chapter(category, f"excel/reference/{slug}.md",
                                                       excel_reference(category, functions), item))
            visit(item["sub_items"])

    visit(book["items"])
    if found != {"python/api.md", "excel/reference.md"}:
        raise ValueError("The book must include Python API and Excel reference chapters")
    return book


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "supports":
        return 0 if sys.argv[2] == "html" else 1
    context, book = json.load(sys.stdin)
    json.dump(preprocess(context, book), sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())