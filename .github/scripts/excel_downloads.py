"""Select released source and stage one complete Excel release for Pages."""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import tomllib
from zipfile import BadZipFile, ZipFile


ASSETS = (
    "quantsupport.xll",
    "quantsupport.intellisense.xml",
    "ExcelDna.IntelliSense64.xll",
    "quantsupport-excel-windows-x64.zip",
)


def command(*arguments):
    result = subprocess.run(arguments, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or f"Command failed: {arguments[0]}")
    return result.stdout.strip()


def resolve(repository, run_id=None, tag=None):
    """A release run reserves its version in a signed commit with this trailer."""
    if run_id:
        if not run_id.isdecimal():
            raise ValueError("Expected a numeric Release workflow run ID")
        if tag:
            raise ValueError("Select either a Release workflow run or a tag")
        ref = command("git", "log", "origin/main", "--format=%H", "-1",
                      f"--grep=^Release-Run-ID: {run_id}$")
        if not ref:
            raise RuntimeError(f"No version commit found for Release workflow {run_id}")
        manifest = tomllib.loads(command("git", "show", f"{ref}:Cargo.toml"))
        tag = "v" + manifest["workspace"]["package"]["version"]
    else:
        ref = None
        if not tag:
            tag = json.loads(command("gh", "release", "view", "--repo", repository,
                                     "--json", "tagName"))["tagName"]
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", tag):
        raise ValueError(f"Invalid release tag: {tag!r}")
    tagged_ref = command("git", "rev-parse", f"refs/tags/{tag}^{{commit}}")
    if ref and tagged_ref != ref:
        raise RuntimeError(f"{tag} does not point to Release workflow {run_id}'s version commit")
    return {"ref": tagged_ref, "tag": tag}


def stage(repository, tag, destination, examples):
    """Never combine a current checkout's metadata with an older released XLL."""
    release = json.loads(command("gh", "release", "view", tag, "--repo", repository,
                                 "--json", "tagName,assets"))
    if release["tagName"] != tag:
        raise RuntimeError(f"Requested {tag}, received release {release['tagName']}")
    available = {asset["name"] for asset in release["assets"]
                 if asset.get("size", 0) > 0 and asset.get("state") == "uploaded"}
    missing = sorted(set(ASSETS) - available)
    if missing:
        raise RuntimeError(f"Release {tag} is missing Excel assets: {', '.join(missing)}. "
                           "Wait for the Release workflow to finish or select a complete release.")
    print(f"Staging Excel downloads from {tag}", flush=True)
    with TemporaryDirectory(prefix="quantsupport-downloads-") as temporary:
        arguments = ["gh", "release", "download", tag, "--repo", repository, "--dir", temporary]
        for asset in ASSETS:
            arguments += ["--pattern", asset]
        command(*arguments)
        downloaded = Path(temporary)
        for asset in ASSETS:
            path = downloaded / asset
            if not path.is_file() or path.stat().st_size == 0:
                raise RuntimeError(f"Release {tag}: download missing or empty: {asset}")
        try:
            with ZipFile(downloaded / ASSETS[-1]) as archive:
                for asset in ASSETS[:-1]:
                    if asset not in archive.namelist():
                        raise RuntimeError(f"Release {tag}: ZIP is missing {asset}")
                    if archive.read(asset) != (downloaded / asset).read_bytes():
                        raise RuntimeError(f"Release {tag}: ZIP and standalone {asset} do not match")
        except BadZipFile as error:
            raise RuntimeError(f"Release {tag}: invalid Excel ZIP: {error}") from error
        workbooks = list(Path(examples).glob("*.xlsx"))
        if not workbooks:
            raise RuntimeError(f"No example workbooks found in {examples}")
        destination = Path(destination)
        destination.mkdir(parents=True, exist_ok=True)
        for asset in ASSETS:
            shutil.copy2(downloaded / asset, destination / asset)
        for workbook in workbooks:
            shutil.copy2(workbook, destination / workbook.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    selection = actions.add_parser("resolve")
    selection.add_argument("--repository", required=True)
    selection.add_argument("--run-id")
    selection.add_argument("--tag")
    download = actions.add_parser("stage")
    download.add_argument("--repository", required=True)
    download.add_argument("--tag", required=True)
    download.add_argument("--destination", type=Path, required=True)
    download.add_argument("--examples", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.action == "resolve":
            selected = resolve(args.repository, args.run_id, args.tag)
            print(f"Selected {selected['tag']} ({selected['ref']})", flush=True)
            with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
                for key, value in selected.items():
                    output.write(f"{key}={value}\n")
        else:
            stage(args.repository, args.tag, args.destination, args.examples)
    except (RuntimeError, ValueError, OSError) as error:
        print(f"Excel download staging failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
