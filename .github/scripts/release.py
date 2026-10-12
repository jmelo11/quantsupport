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


def run_tests():
    """Keep tests alongside the release implementation."""
    import io
    import tempfile
    import unittest
    from unittest.mock import patch

    manifest = '[workspace.package]\nversion = "0.1.8"\n\n[dependencies]\nx = "0.1.8"\n'

    class ReleaseTests(unittest.TestCase):
        def test_retry_finds_its_commit_after_another_main_push(self):
            with tempfile.TemporaryDirectory() as directory:
                remote = Path(directory) / "remote.git"
                checkout = Path(directory) / "checkout"
                subprocess.run(
                    [
                        "git",
                        "init",
                        "--quiet",
                        "--bare",
                        "--initial-branch=main",
                        str(remote),
                    ],
                    check=True,
                )
                subprocess.run(
                    ["git", "clone", "--quiet", str(remote), str(checkout)],
                    check=True,
                    stderr=subprocess.DEVNULL,
                )

                def local_git(*args):
                    return subprocess.check_output(
                        ["git", *args],
                        cwd=checkout,
                        text=True,
                    ).rstrip("\n")

                local_git("config", "user.name", "Release test")
                local_git("config", "user.email", "release-test@example.invalid")
                (checkout / "Cargo.toml").write_text(manifest)
                local_git("add", "Cargo.toml")
                local_git("commit", "--quiet", "-m", "Initial version")
                updated, version = bump_manifest(manifest, [])
                (checkout / "Cargo.toml").write_text(updated)
                local_git(
                    "commit", "--quiet", "-am", "Version", "-m", "Release-Run-ID: 42"
                )
                reserved = local_git("rev-parse", "HEAD")
                local_git("tag", f"v{version}")
                (checkout / "README.md").write_text("A later push\n")
                local_git("add", "README.md")
                local_git("commit", "--quiet", "-m", "Later push")
                local_git("push", "--quiet", "origin", "main", "--tags")
                with patch("__main__.git", side_effect=local_git), patch(
                    "__main__.github_api"
                ) as api:
                    self.assertEqual(prepare("owner/repo", "42"), (version, reserved))
                    api.assert_not_called()

        def test_shared_version_and_newer_reserved_tag(self):
            updated, version = bump_manifest(
                manifest, ["v0.1.7-alpha", "v0.1.9", "unrelated"]
            )
            self.assertEqual(version, "0.1.10")
            data = tomllib.loads(updated)
            self.assertEqual(data["workspace"]["package"]["version"], "0.1.10")
            self.assertEqual(data["dependencies"]["x"], "0.1.8")

        def test_retry_reuses_version_after_commit_but_before_tag(self):
            def fake_git(*args):
                if args[0] == "log":
                    return "reserved-commit"
                if args[0] == "show":
                    return manifest
                return ""

            with patch("__main__.git", side_effect=fake_git), patch(
                "__main__.github_api"
            ) as api:
                self.assertEqual(
                    prepare("owner/repo", "42"), ("0.1.8", "reserved-commit")
                )
                api.assert_called_once_with(
                    "repos/owner/repo/git/refs",
                    {
                        "ref": "refs/tags/v0.1.8",
                        "sha": "reserved-commit",
                    },
                )

        def test_existing_tag_must_match_release_commit(self):
            with patch("__main__.git", side_effect=["v0.1.8", "different-commit"]):
                with self.assertRaisesRegex(RuntimeError, "another commit"):
                    ensure_tag("owner/repo", "0.1.8", "reserved-commit")

        def test_concurrent_push_retries_without_overwriting_it(self):
            state = {"head": "first-commit", "attempts": 0}

            def fake_git(*args):
                if args[0] == "rev-parse":
                    return state["head"]
                if args[0] == "show":
                    return manifest.strip()
                return ""

            def fake_api(endpoint, payload):
                if endpoint != "graphql":
                    return {}
                state["attempts"] += 1
                expected = payload["variables"]["input"]["expectedHeadOid"]
                if state["attempts"] == 1:
                    self.assertEqual(expected, "first-commit")
                    state["head"] = "concurrent-push"
                    raise subprocess.CalledProcessError(1, "gh", stderr="head changed")
                self.assertEqual(expected, "concurrent-push")
                return {
                    "data": {
                        "createCommitOnBranch": {"commit": {"oid": "release-commit"}}
                    }
                }

            with patch("__main__.git", side_effect=fake_git), patch(
                "__main__.github_api", side_effect=fake_api
            ):
                self.assertEqual(
                    prepare("owner/repo", "42"), ("0.1.9", "release-commit")
                )
                self.assertEqual(state["attempts"], 2)

        def test_permission_error_does_not_keep_retrying(self):
            def fake_git(*args):
                if args[0] == "rev-parse":
                    return "same-head"
                if args[0] == "show":
                    return manifest.strip()
                return ""

            error = subprocess.CalledProcessError(1, "gh", stderr="protected branch")
            with patch("__main__.git", side_effect=fake_git), patch(
                "__main__.github_api", side_effect=error
            ) as api:
                with self.assertRaisesRegex(RuntimeError, "RELEASE_TOKEN"):
                    prepare("owner/repo", "42")
                self.assertEqual(api.call_count, 1)

        def test_registry_only_treats_not_found_as_unpublished(self):
            missing = HTTPError("url", 404, "missing", {}, None)
            with patch("__main__.urlopen", side_effect=missing):
                self.assertFalse(is_published("0.1.9"))
            unavailable = HTTPError("url", 503, "unavailable", {}, None)
            with patch("__main__.urlopen", side_effect=unavailable):
                with self.assertRaises(HTTPError):
                    is_published("0.1.9")
            with patch("__main__.urlopen", return_value=io.StringIO('{"version": {}}')):
                self.assertTrue(is_published("0.1.9"))

    result = unittest.TextTestRunner(verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(ReleaseTests)
    )
    if not result.wasSuccessful():
        raise SystemExit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "published", "test"))
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
    else:
        run_tests()
