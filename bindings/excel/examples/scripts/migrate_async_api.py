"""Migrate the previous explicit-input worksheet API, preserving workbook layout.

Usage: python migrate_async_api.py FILE.xlsx [...]
Keep a copy of external workbooks before migrating; both XLLs must match the
new API. Source examples are migrated in place as part of this release.
"""
import argparse
import html
from pathlib import Path
import re
from zipfile import ZipFile

FUNCTION = re.compile(r"(?:_xll\.)?QS\.(?:CONTEXT\.PREPARE|EVALUATE(?:\.RESULTS|\.RANGE)?|SNAPSHOT\.FROM\.CONTEXT)\(")

def rewrite_formula(expression):
    result = ""
    start = 0
    while match := FUNCTION.search(expression, start):
        depth, quoted, args, current = 0, False, [], ""
        index = match.end()
        while index < len(expression):
            char = expression[index]
            if char == '"': quoted = not quoted
            if not quoted:
                if char in "({": depth += 1
                elif char == "}": depth -= 1
                elif char == ")":
                    if depth == 0:
                        args.append(current)
                        break
                    depth -= 1
                elif char == "," and depth == 0:
                    args.append(current)
                    current = ""
                    index += 1
                    continue
            current += char
            index += 1
        else: raise ValueError(f"Unclosed formula: {expression}")
        result += expression[start:match.start()]
        if "CONTEXT.PREPARE" in match[0]:
            replacement = args[1]
        else:
            if len(args) >= 4: del args[3]
            replacement = match[0] + ",".join(args) + ")"
        result += replacement
        start = index + 1
    return result + expression[start:]

def migrate(path):
    path = Path(path)
    temporary = path.with_suffix(".async.tmp")
    try:
        with ZipFile(path) as source, ZipFile(temporary, "w") as target:
            for info in source.infolist():
                data = source.read(info.filename)
                if info.filename.startswith("xl/worksheets/") and info.filename.endswith(".xml") or info.filename == "xl/sharedStrings.xml":
                    text = data.decode("utf-8")
                    def rewrite(match):
                        content = html.unescape(match[2])
                        if "QS." not in content: return match[0]
                        return match[1] + html.escape(rewrite_formula(content), quote=False) + match[3]
                    text = re.sub(r"(<(?:f|t)(?:\s[^>]*)?>)(.*?)(</(?:f|t)>)", rewrite, text, flags=re.S)
                    text = text.replace("complete input range", "context-owned market definitions").replace("Prepare market", "Use root market")
                    data = text.encode("utf-8")
                target.writestr(info, data)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("workbooks", nargs="+")
    for path in parser.parse_args().workbooks: migrate(path)
