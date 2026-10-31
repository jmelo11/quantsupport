"""Prepare repeatable releases and check crates.io publication status."""

import argparse
import base64
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def git(*args):
    return subprocess.check_output(["git", *args], text=True).rstrip("\n")


def github_api(endpoint, payload=None):
    command = ["gh", "api", endpoint]
    if payload is not None:
        command.extend(["--input", "-"])
    result = subprocess.run(
        command,
        input=json.dumps(payload) if payload is not None else None,
        text=True,
        capture_output=True,
        check=True,
    )
    return json.loads(result.stdout)


def bump_manifest(manifest, tags):
    """Increment the highest stable version without changing other TOML fields."""
    current = tomllib.loads(manifest)["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", current):
        raise ValueError(f"Expected a stable workspace version, got {current!r}")
    versions = [tuple(map(int, current.split(".")))]
    for tag in tags:
        if re.fullmatch(r"v\d+\.\d+\.\d+", tag):
            versions.append(tuple(map(int, tag[1:].split("."))))
    major, minor, patch = max(versions)
    version = f"{major}.{minor}.{patch + 1}"
    section = re.search(r"(?ms)^\[workspace\.package\]\s*\n.*?(?=^\[|\Z)", manifest)
    if section is None:
        raise ValueError("Missing [workspace.package] section")
    updated, count = re.subn(
        r"(?m)^(version\s*=\s*)[\'\"][^\'\"]+[\'\"]",
        lambda match: f'{match[1]}"{version}"',
        section[0],
    )
    if count != 1:
        raise ValueError("Expected one workspace version assignment")
    manifest = manifest[: section.start()] + updated + manifest[section.end() :]
    return manifest, version


def ensure_tag(repository, version, commit):
    tag = f"v{version}"
    if tag in git("tag", "--list").splitlines():
        if git("rev-parse", f"{tag}^{{commit}}") != commit:
            raise RuntimeError(f"{tag} already points to another commit")
        return
    github_api(
        f"repos/{repository}/git/refs", {"ref": f"refs/tags/{tag}", "sha": commit}
    )


def prepare(repository, run_id):
    """Reserve one version per workflow run, including when a run is retried."""
    if not re.fullmatch(r"\d+", run_id):
        raise ValueError("Expected a numeric GitHub run ID")
    for _ in range(5):
        git("fetch", "--quiet", "origin", "main", "--tags")
        previous = git(
            "log",
            "origin/main",
            "--format=%H",
            "-1",
            f"--grep=^Release-Run-ID: {run_id}$",
        )
        if previous:
            manifest = git("show", f"{previous}:Cargo.toml")
            version = tomllib.loads(manifest)["workspace"]["package"]["version"]
            ensure_tag(repository, version, previous)
            return version, previous

        head = git("rev-parse", "origin/main")
        manifest = git("show", "origin/main:Cargo.toml") + "\n"
        updated, version = bump_manifest(manifest, git("tag", "--list").splitlines())
        # GitHub signs commits made through this mutation. expectedHeadOid prevents
        # overwriting a push that reaches main while the version is being prepared.
        payload = {
            "query": """
                mutation($input: CreateCommitOnBranchInput!) {
                    createCommitOnBranch(input: $input) { commit { oid } }
                }
            """,
            "variables": {
                "input": {
                    "branch": {
                        "repositoryNameWithOwner": repository,
                        "branchName": "main",
                    },
                    "expectedHeadOid": head,
                    "message": {
                        "headline": f"chore(release): v{version} [skip release]",
                        "body": f"Release-Run-ID: {run_id}",
                    },
                    "fileChanges": {
                        "additions": [
                            {
                                "path": "Cargo.toml",
                                "contents": base64.b64encode(updated.encode()).decode(),
                            }
                        ]
                    },
                }
            },
        }
        try:
            response = github_api("graphql", payload)
        except subprocess.CalledProcessError as error:
            git("fetch", "--quiet", "origin", "main")
            if git("rev-parse", "origin/main") != head:
                continue
            raise RuntimeError(
                "Cannot create the signed version commit. On protected main, configure "
                "RELEASE_TOKEN with Contents: write and permission to bypass the PR rule.\n"
                + (error.stderr or str(error))
            ) from error
        commit = response["data"]["createCommitOnBranch"]["commit"]["oid"]
        ensure_tag(repository, version, commit)
        return version, commit
    raise RuntimeError(
        "main changed during all five attempts; rerun the Release workflow"
    )


def is_published(version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Expected a stable release version")
    request = Request(
        f"https://crates.io/api/v1/crates/quantsupport/{version}",
        headers={
            "User-Agent": "quantsupport-release (https://github.com/jmelo11/quantsupport)"
        },
    )
    try:
        with urlopen(request, timeout=30) as response:
            json.load(response)
        return True
    except HTTPError as error:
        if error.code == 404:
            return False
        raise


def outputs(**values):
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        for key, value in values.items():
            output.write(f"{key}={value}\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "published"))
    parser.add_argument("version", nargs="?")
    args = parser.parse_args()
    if args.command == "prepare":
        version, commit = prepare(
            os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_RUN_ID"]
        )
        outputs(version=version, ref=commit)
        print(f"Prepared v{version} at {commit}")
    elif args.command == "published":
        if args.version is None:
            parser.error("published requires a version")
        outputs(exists=str(is_published(args.version)).lower())
