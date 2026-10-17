"""Resolve PyO3 constructors from their own stubs, never inherited __init__."""

import ast
import copy
from functools import lru_cache
from pathlib import Path


@lru_cache
def declarations():
    root = Path(__file__).resolve().parents[2] / "bindings/python/python/quantsupport"
    result = {}
    aliases = {"PythonDate": ast.parse("datetime.date", mode="eval").body}
    for path in root.rglob("*.pyi"):
        module = ".".join(("quantsupport", *path.relative_to(root).parts[:-1]))
        tree = ast.parse(path.read_text())
        for node in tree.body:
            if isinstance(node, ast.Assign) and len(node.targets) == 1:
                target = node.targets[0]
                if isinstance(target, ast.Name) and target.id.startswith("_") and not target.id.startswith("__"):
                    aliases[target.id] = node.value
        for node in tree.body:
            if isinstance(node, ast.ClassDef):
                result[f"{module}.{node.name}"] = (node, aliases)
    return result


def own_constructor(node):
    methods = {member.name: member for member in node.body if isinstance(member, ast.FunctionDef)}
    return methods.get("__new__", methods.get("__init__"))


class PublicAliases(ast.NodeTransformer):
    def __init__(self, aliases):
        self.aliases = aliases

    def visit_Name(self, node):
        if node.id in self.aliases:
            return self.visit(copy.deepcopy(self.aliases[node.id]))
        return node


def constructor_signature(obj):
    entry = declarations().get(obj.id)
    if entry is None:
        return obj.args
    node, aliases = entry
    constructor = own_constructor(node)
    if constructor is None:
        return ""
    args = copy.deepcopy(constructor.args)
    positional = args.posonlyargs or args.args
    if positional and positional[0].arg in ("self", "cls"):
        positional.pop(0)
    return ast.unparse(PublicAliases(aliases).visit(args))


def public_signature(args):
    parsed = ast.parse(f"def declaration({args}): pass").body[0]
    aliases = next(iter(declarations().values()))[1]
    return ast.unparse(PublicAliases(aliases).visit(parsed.args))


def method_signature(obj):
    owner, name = obj.id.rsplit(".", 1)
    entry = declarations().get(owner)
    if entry:
        cls, aliases = entry
        for method in cls.body:
            if isinstance(method, ast.FunctionDef) and method.name == name:
                args = copy.deepcopy(method.args)
                positional = args.posonlyargs or args.args
                if positional and positional[0].arg in ("self", "cls"):
                    positional.pop(0)
                return ast.unparse(PublicAliases(aliases).visit(args))
    return public_signature(obj.args)


def declaration_source(obj):
    node, _ = declarations()[obj.id]
    module = obj.id.rsplit(".", 1)[0].replace(".", "/")
    return f"https://github.com/jmelo11/quantsupport/blob/main/bindings/python/python/{module}/__init__.pyi#L{node.lineno}"


def prepare_class_docstrings(app, what, name, obj, options, lines):
    if what not in ("class", "exception") or name not in declarations():
        return
    node, _ = declarations()[name]
    constructor = own_constructor(node)
    docs = [ast.get_docstring(node) or ""]
    if constructor is not None and ast.get_docstring(constructor):
        docs.append(ast.get_docstring(constructor))
    lines[:] = "\n\n".join(docs).splitlines() + [""]