"""Stage Python release assets and generate their standalone hosted page."""

import argparse
from html import escape
import json
import os
from pathlib import Path
import re
import shutil
from tempfile import TemporaryDirectory
from urllib.parse import quote

from excel_downloads import command


PYTHONS = ("3.11", "3.12", "3.13", "3.14")
PLATFORMS = (
    ("Windows", "Intel / AMD x64"), ("Windows", "ARM64"),
    ("macOS", "Intel x64"), ("macOS", "Apple silicon ARM64"),
    ("Linux", "Intel / AMD x64"), ("Linux", "ARM64"),
)


def version(tag):
    if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        raise ValueError(f"Expected a stable release tag, got {tag!r}")
    return tuple(map(int, tag[1:].split(".")))


def packages(release):
    """Read actual uploaded assets; never invent links for missing packages."""
    tag = release["tagName"]
    version(tag)
    available = {asset["name"] for asset in release["assets"]
                 if asset.get("size", 0) > 0 and asset.get("state") == "uploaded"}
    result = {}
    for name in sorted(available):
        match = re.fullmatch(
            rf"quantsupport-{re.escape(tag[1:])}-cp(311|312|313|314)-cp\1-(.+)\.whl", name,
        )
        if not match:
            continue
        python = f"3.{match[1][1:]}"
        platform = match[2]
        if platform == "win_amd64":
            target = PLATFORMS[0]
        elif platform == "win_arm64":
            target = PLATFORMS[1]
        elif platform.startswith("macosx_") and platform.endswith("_x86_64"):
            target = PLATFORMS[2]
        elif platform.startswith("macosx_") and platform.endswith("_arm64"):
            target = PLATFORMS[3]
        elif platform.startswith("manylinux_") and platform.endswith("_x86_64"):
            target = PLATFORMS[4]
        elif platform.startswith("manylinux_") and platform.endswith("_aarch64"):
            target = PLATFORMS[5]
        else:
            continue
        key = (*target, python)
        if key in result:
            raise RuntimeError(f"Release {tag}: multiple wheels for {key}")
        portable = name.removesuffix(".whl") + "-portable.zip"
        result[key] = {"wheel": name, "portable": portable if portable in available else None}
    return result


def release(repository, tag):
    result = json.loads(command("gh", "release", "view", tag, "--repo", repository,
                                "--json", "tagName,assets"))
    if result["tagName"] != tag:
        raise RuntimeError(f"Requested {tag}, received {result['tagName']}")
    return result


def select(repository, tag):
    """Anchor to the deployed source, even if another release has just finished."""
    current = release(repository, tag)
    current_packages = packages(current)
    missing = [(os, arch, python) for os, arch in PLATFORMS for python in PYTHONS
               if not current_packages.get((os, arch, python), {}).get("portable")]
    if missing:
        raise RuntimeError(f"Release {tag} is missing Python wheels or folder ZIPs: {missing}")
    listing = json.loads(command(
        "gh", "release", "list", "--repo", repository, "--limit", "100",
        "--exclude-drafts", "--exclude-pre-releases", "--json", "tagName",
    ))
    candidates = [item["tagName"] for item in listing
                  if re.fullmatch(r"v\d+\.\d+\.\d+", item["tagName"])
                  and version(item["tagName"]) < version(tag)]
    selected = [(tag, current_packages)]
    for previous in sorted(set(candidates), key=version, reverse=True):
        previous_packages = packages(release(repository, previous))
        if previous_packages:
            selected.append((previous, previous_packages))
            break
    return selected


def render(repository, selected, local_prefix=None):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Expected repository owner/name")
    tables = []
    for tag, entries in selected:
        rows = []
        for os, arch in PLATFORMS:
            cells = []
            for python in PYTHONS:
                entry = entries.get((os, arch, python))
                links = []
                for kind, label in (("portable", "Folder ZIP"), ("wheel", "Wheel")):
                    if entry and entry.get(kind):
                        if local_prefix is not None and tag == selected[0][0]:
                            url = f"{local_prefix}/{quote(tag, safe='')}/{quote(entry[kind], safe='')}"
                        else:
                            url = (f"https://github.com/{repository}/releases/download/"
                                   f"{quote(tag, safe='')}/{quote(entry[kind], safe='')}")
                        links.append(f'<a href="{escape(url, quote=True)}">{label}</a>')
                cells.append("<td>" + ("<br>".join(links) or "Unavailable") + "</td>")
            rows.append(f'<tr><th scope="row">{escape(os)} · {escape(arch)}</th>'
                        + "".join(cells) + "</tr>")
        headers = '<th scope="col">Operating system / architecture</th>' + "".join(
            f'<th scope="col">Python {python}</th>' for python in PYTHONS)
        tables.append(f'<h2 id="{tag}">{escape(tag)}</h2>\n'
                      f'<div style="overflow-x:auto"><table><caption>QuantSupport {escape(tag)}</caption>'
                      f'<thead><tr>{headers}</tr></thead><tbody>{"".join(rows)}</tbody></table></div>')
    return "\n".join(tables)


def stage(repository, tag, page, destination):
    page = Path(page)
    destination = Path(destination)
    selected = select(repository, tag)
    names = sorted({name for entry in selected[0][1].values()
                    for name in entry.values() if name})
    with TemporaryDirectory(prefix="quantsupport-python-assets-") as temporary:
        arguments = ["gh", "release", "download", tag, "--repo", repository, "--dir", temporary]
        for name in names:
            arguments.extend(["--pattern", name])
        command(*arguments)
        downloaded = Path(temporary)
        for name in names:
            asset = downloaded / name
            if not asset.is_file() or asset.stat().st_size == 0:
                raise RuntimeError(f"Release {tag}: download missing or empty: {name}")
        # Stage the selected release alongside Excel's files. Previous releases
        # remain hosted on GitHub so Pages retains one set of large binaries.
        release_directory = destination / tag
        release_directory.mkdir(parents=True, exist_ok=True)
        for name in names:
            shutil.copy2(downloaded / name, release_directory / name)
    prefix = Path(os.path.relpath(destination, page.parent)).as_posix()
    tables = render(repository, selected, local_prefix=prefix)
    source = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>QuantSupport Python packages</title>
<style>
body { font-family: system-ui, sans-serif; max-width: 72rem; margin: 3rem auto; padding: 0 1rem; }
table { border-collapse: collapse; width: 100%; margin-bottom: 2rem; }
th, td { padding: .75rem; border-bottom: 1px solid #ddd; text-align: left; }
caption { text-align: left; margin-bottom: .75rem; }
a { color: #1763a6; }
</style>
</head>
<body><h1>QuantSupport Python packages</h1>
""" + tables + "\n</body>\n</html>\n"
    page.parent.mkdir(parents=True, exist_ok=True)
    page.write_text(source, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--page", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    stage(args.repository, args.tag, args.page, args.destination)


if __name__ == "__main__":
    main()
