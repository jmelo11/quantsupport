"""Regression checks for release/Pages ordering and complete Excel downloads."""

import io
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from zipfile import ZipFile

import excel_downloads as downloads


class ReleaseSelectionTests(unittest.TestCase):
    def setUp(self):
        directory = TemporaryDirectory(prefix="release-selection-")
        self.addCleanup(directory.cleanup)
        self.repository = Path(directory.name)
        self.git("init", "--quiet", "--initial-branch=main")
        self.git("config", "user.name", "Release test")
        self.git("config", "user.email", "release-test@example.invalid")
        self.first = self.reserve("0.1.12", "42")
        self.latest = self.reserve("0.1.13", "99")
        self.git("update-ref", "refs/remotes/origin/main", self.latest)

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repository, text=True).strip()

    def reserve(self, version, run):
        (self.repository / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')
        self.git("add", "Cargo.toml")
        self.git("commit", "--quiet", "-m", f"Version {version}", "-m", f"Release-Run-ID: {run}")
        self.git("tag", f"v{version}")
        return self.git("rev-parse", "HEAD")

    def command(self, *args):
        if args[0] == "git":
            return self.git(*args[1:])
        self.assertEqual(args, ("gh", "release", "view", "--repo", "owner/repo", "--json", "tagName"))
        return json.dumps({"tagName": "v0.1.13"})

    def test_release_run_keeps_its_version_after_a_newer_main_push(self):
        with patch.object(downloads, "command", side_effect=self.command):
            self.assertEqual(downloads.resolve("owner/repo", run_id="42"),
                             {"tag": "v0.1.12", "ref": self.first})

    def test_manual_tag_selects_the_same_source_as_the_downloads(self):
        with patch.object(downloads, "command", side_effect=self.command):
            self.assertEqual(downloads.resolve("owner/repo", tag="v0.1.12"),
                             {"tag": "v0.1.12", "ref": self.first})
            self.assertEqual(downloads.resolve("owner/repo"),
                             {"tag": "v0.1.13", "ref": self.latest})

    def test_missing_version_commit_and_wrong_tag_fail_explicitly(self):
        with patch.object(downloads, "command", side_effect=self.command):
            with self.assertRaisesRegex(RuntimeError, "No version commit found.*123"):
                downloads.resolve("owner/repo", run_id="123")
            self.git("tag", "--force", "v0.1.12", self.latest)
            with self.assertRaisesRegex(RuntimeError, "does not point to Release workflow 42"):
                downloads.resolve("owner/repo", run_id="42")


class DownloadStagingTests(unittest.TestCase):
    def setUp(self):
        directory = TemporaryDirectory(prefix="download-staging-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.destination = self.root / "downloads"
        self.examples = self.root / "examples"
        self.examples.mkdir()
        (self.examples / "swap_pricing_example.xlsx").write_bytes(b"matching release workbook fixture")
        self.payloads = {name: ("Isolated fixture: " + name).encode() for name in downloads.ASSETS[:-1]}
        archive = io.BytesIO()
        with ZipFile(archive, "w") as writer:
            for name, payload in self.payloads.items():
                writer.writestr(name, payload)
        self.payloads[downloads.ASSETS[-1]] = archive.getvalue()
        self.assets = [{"name": name, "size": len(payload), "state": "uploaded"}
                       for name, payload in self.payloads.items()]
        self.calls = []

    def command(self, *args):
        self.calls.append(args)
        self.assertEqual(args[:4], ("gh", "release", args[2], "v0.1.12"))
        if args[2] == "view":
            return json.dumps({"tagName": "v0.1.12", "assets": self.assets})
        self.assertEqual(args[2], "download")
        destination = Path(args[args.index("--dir") + 1])
        for name, payload in self.payloads.items():
            (destination / name).write_bytes(payload)
        return ""

    def stage(self):
        with patch.object(downloads, "command", side_effect=self.command):
            downloads.stage("owner/repo", "v0.1.12", self.destination, self.examples)

    def test_complete_release_stages_all_assets_and_matching_workbooks(self):
        self.stage()
        for name, payload in self.payloads.items():
            self.assertEqual((self.destination / name).read_bytes(), payload)
        self.assertEqual((self.destination / "swap_pricing_example.xlsx").read_bytes(),
                         (self.examples / "swap_pricing_example.xlsx").read_bytes())
        self.assertEqual(len(self.calls), 2)

    def test_previous_xll_only_release_lists_missing_files_without_downloading(self):
        self.assets = self.assets[:1]
        with self.assertRaises(RuntimeError) as raised:
            self.stage()
        message = str(raised.exception)
        for name in downloads.ASSETS[1:]:
            self.assertIn(name, message)
        self.assertIn("Release v0.1.12", message)
        self.assertEqual(len(self.calls), 1)
        self.assertFalse(self.destination.exists())

    def test_empty_or_incomplete_asset_is_reported_as_missing(self):
        self.assets[1]["size"] = 0
        self.assets[2]["state"] = "new"
        with self.assertRaisesRegex(RuntimeError, "missing Excel assets"):
            self.stage()
        self.assertFalse(self.destination.exists())

    def test_downloaded_files_must_all_be_present(self):
        del self.payloads[downloads.ASSETS[1]]
        with self.assertRaisesRegex(RuntimeError, "download missing or empty: quantsupport.intellisense.xml"):
            self.stage()
        self.assertFalse(self.destination.exists())

    def test_mismatched_zip_cannot_replace_downloads(self):
        self.payloads[downloads.ASSETS[1]] = b"different metadata"
        with self.assertRaisesRegex(RuntimeError, "ZIP and standalone quantsupport.intellisense.xml do not match"):
            self.stage()
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
