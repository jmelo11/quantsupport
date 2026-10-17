"""Sphinx worksheet domain and symbol pages from the native registration catalog."""

from collections import defaultdict
import json
from pathlib import Path
import subprocess

from docutils import nodes
from sphinx import addnodes
from sphinx.directives import ObjectDescription
from sphinx.domains import Domain, Index, ObjType
from sphinx.errors import ExtensionError
from sphinx.roles import XRefRole
from sphinx.util.docfields import Field, TypedField
from sphinx.util.nodes import make_refnode
from api_navigation import write_excel_navigation


class WorksheetFunction(ObjectDescription):
    doc_field_types = [
        TypedField("parameter", label="Parameters", names=("param",),
                   typenames=("type",), can_collapse=False),
        Field("returns", label="Returns", names=("returns",), has_arg=False),
        Field("returntype", label="Return Type", names=("rtype",), has_arg=False),
        Field("errors", label="Errors", names=("errors",), has_arg=False),
    ]

    def handle_signature(self, sig, signode):
        name, rest = sig.split("(", 1)
        parameters, result = rest.rsplit(")", 1)
        signode += addnodes.desc_name(name, name)
        parameter_list = addnodes.desc_parameterlist()
        for parameter in filter(None, parameters.split(",")):
            parameter_list += addnodes.desc_parameter(parameter.strip(), parameter.strip())
        signode += parameter_list
        if result.strip():
            returns = result.strip().removeprefix("->").strip()
            signode += addnodes.desc_returns(returns, returns)
        return name

    def add_target_and_index(self, name, sig, signode):
        target = f"excel-{name.lower().replace('.', '-')}"
        signode["ids"].append(target)
        self.state.document.note_explicit_target(signode)
        self.env.get_domain("excel").note_function(name, self.env.docname, target)
        self.indexnode["entries"].append(("single", name, target, "", None))


class FunctionIndex(Index):
    name = "functionindex"
    localname = "Excel Function Index"
    shortname = "Excel functions"

    def generate(self, docnames=None):
        groups = defaultdict(list)
        for name, (docname, target) in sorted(self.domain.data["functions"].items()):
            if docnames is None or docname in docnames:
                groups[name[3:4].lower()].append((name, 0, docname, target, "", "", ""))
        return sorted(groups.items()), False


class WorksheetDomain(Domain):
    name = "excel"
    label = "Excel"
    object_types = {"function": ObjType("worksheet function", "func")}
    directives = {"function": WorksheetFunction}
    roles = {"func": XRefRole()}
    indices = [FunctionIndex]
    initial_data = {"functions": {}}

    def note_function(self, name, docname, target):
        self.data["functions"][name] = (docname, target)

    def clear_doc(self, docname):
        self.data["functions"] = {name: value for name, value in self.data["functions"].items()
                                  if value[0] != docname}

    def merge_domaindata(self, docnames, otherdata):
        self.data["functions"].update({name: value for name, value in otherdata["functions"].items()
                                       if value[0] in docnames})

    def get_objects(self):
        for name, (docname, target) in sorted(self.data["functions"].items()):
            yield name, name, "function", docname, target, 1

    def resolve_xref(self, env, fromdocname, builder, typ, target, node, contnode):
        if target in self.data["functions"]:
            docname, anchor = self.data["functions"][target]
            return make_refnode(builder, fromdocname, docname, anchor, contnode, target)
        return None


def worksheet_type(native):
    compact = "".join(native.split())
    if compact.startswith("Option<") and compact.endswith(">"):
        return f"{worksheet_type(compact[7:-1])} or blank"
    if "XLOPER12" in compact:
        return "cell or range"
    if "str" in compact or "String" in compact:
        return "text"
    if compact == "bool":
        return "logical"
    if compact in ("i32", "i64", "u32", "usize"):
        return "integer"
    if compact == "f64":
        return "number"
    raise ExtensionError(f"Unknown worksheet input type: {native}")


def result_type(record):
    native = "".join(record["return_type"].split())
    if "XLOPER12" in native or "Vec<" in native:
        return "spilled range"
    if "String" in native:
        return "text handle" if "handle" in record["returns"].lower() else "text"
    if "bool" in native:
        return "logical"
    if "f64" in native:
        return "number"
    if "i32" in native:
        return "integer"
    raise ExtensionError(f"Unknown worksheet result type: {record['return_type']}")


def function_page(record):
    name = record["name"]
    declaration = ", ".join(f"{argument['name']}: {worksheet_type(argument['type'])}"
                            for argument in record["arguments"])
    lines = [name, "=" * len(name), "", record["help"], "",
             f".. excel:function:: {name}({declaration}) -> {result_type(record)}", ""]
    for argument in record["arguments"]:
        lines.extend([f"   :param {argument['name']}: {argument['help']}",
                      f"   :type {argument['name']}: {worksheet_type(argument['type'])}"])
    lines.extend([f"   :returns: {record['returns']}", f"   :rtype: {result_type(record)}",
                  f"   :errors: {record['errors']}", ""])
    if record["aliases"]:
        lines.extend(["Aliases: " + ", ".join(f"``{alias}``" for alias in record["aliases"]) + ".", ""])
    lines.extend(["Formula", "-------", "", ".. code-block:: text", "",
                  f"   ={name}({', '.join(argument['name'] for argument in record['arguments'])})", "",
                  "Calculation", "-----------", "",
                  "Thread-safe scalar function." if record["threadsafe"] else "Runs on Excel's main calculation thread.", ""])
    if record["volatile"]:
        lines.extend(["Volatile registration. Unchanged inputs reuse the current cached object; changed inputs advance its revision.", ""])
    lines.extend(["Native Declaration", "------------------", "", ".. code-block:: rust", "",
                  f"   {record['declaration']}", "",
                  f"`Rust source <https://github.com/jmelo11/quantsupport/blob/main/{record['source']}>`_", ""])
    return "\n".join(lines)


def generate(app):
    root = Path(app.confdir).parent
    result = subprocess.run(["cargo", "run", "--quiet", "--package", "quantsupport-xll",
                             "--example", "function_catalog"], cwd=root, text=True, capture_output=True)
    if result.returncode:
        raise ExtensionError(f"Excel catalog generation failed:\n{result.stderr}")
    records = json.loads(result.stdout)
    if not records:
        raise ExtensionError("The Excel function catalog is empty")
    output = Path(app.srcdir) / "excel" / "functions"
    output.mkdir(parents=True, exist_ok=True)
    for previous in output.glob("*.rst"):
        previous.unlink()
    for record in records:
        for field in ("return_type", "returns", "errors", "declaration"):
            if not record.get(field):
                raise ExtensionError(f"Missing {field} for {record['name']}")
        slug = record["name"].lower().replace(".", "-")
        (output / f"{slug}.rst").write_text(function_page(record), encoding="utf-8")
    write_excel_navigation(output, records)


def setup(app):
    app.add_domain(WorksheetDomain)
    app.connect("builder-inited", generate)
    return {"version": "1", "parallel_read_safe": True, "parallel_write_safe": True}