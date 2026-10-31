"""Checks for version reservation, release retries and registry lookups."""

import io
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools/release"))

import release


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
            updated, version = release.bump_manifest(manifest, [])
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
            with patch("release.git", side_effect=local_git), patch(
                "release.github_api"
            ) as api:
                self.assertEqual(release.prepare("owner/repo", "42"), (version, reserved))
                api.assert_not_called()

    def test_shared_version_and_newer_reserved_tag(self):
        updated, version = release.bump_manifest(
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

        with patch("release.git", side_effect=fake_git), patch(
            "release.github_api"
        ) as api:
            self.assertEqual(
                release.prepare("owner/repo", "42"), ("0.1.8", "reserved-commit")
            )
            api.assert_called_once_with(
                "repos/owner/repo/git/refs",
                {
                    "ref": "refs/tags/v0.1.8",
                    "sha": "reserved-commit",
                },
            )

    def test_existing_tag_must_match_release_commit(self):
        with patch("release.git", side_effect=["v0.1.8", "different-commit"]):
            with self.assertRaisesRegex(RuntimeError, "another commit"):
                release.ensure_tag("owner/repo", "0.1.8", "reserved-commit")

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

        with patch("release.git", side_effect=fake_git), patch(
            "release.github_api", side_effect=fake_api
        ):
            self.assertEqual(
                release.prepare("owner/repo", "42"), ("0.1.9", "release-commit")
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
        with patch("release.git", side_effect=fake_git), patch(
            "release.github_api", side_effect=error
        ) as api:
            with self.assertRaisesRegex(RuntimeError, "RELEASE_TOKEN"):
                release.prepare("owner/repo", "42")
            self.assertEqual(api.call_count, 1)

    def test_registry_only_treats_not_found_as_unpublished(self):
        missing = HTTPError("url", 404, "missing", {}, None)
        with patch("release.urlopen", side_effect=missing):
            self.assertFalse(release.is_published("0.1.9"))
        unavailable = HTTPError("url", 503, "unavailable", {}, None)
        with patch("release.urlopen", side_effect=unavailable):
            with self.assertRaises(HTTPError):
                release.is_published("0.1.9")
        with patch("release.urlopen", return_value=io.StringIO('{"version": {}}')):
            self.assertTrue(release.is_published("0.1.9"))


if __name__ == "__main__":
    unittest.main()
