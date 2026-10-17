"""Unit and real-kernel regression checks for the notebook runner."""

import json
import os
from pathlib import Path
import shutil
import sys
from unittest import TestCase, main
from unittest.mock import MagicMock, patch
from uuid import uuid4

import nbformat

import run_notebooks as runner


class KernelEnvironmentTests(TestCase):
    def setUp(self):
        self.identity = {
            "executable": "/environment/bin/python",
            "prefix": "/environment",
            "python": "3.12.13",
            "quantsupport_file": "/environment/quantsupport/__init__.py",
            "quantsupport_version": "0.1.10",
            "FixedRateCoupon": True,
            "Shock": True,
            "Calendar_TARGET": True,
        }
        self.notebook = nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell("1 + 1")]
        )
        self.manager = MagicMock()
        self.client = MagicMock()
        self.path = runner.EXAMPLES / "bootstrap" / "curves.ipynb"
        self.probe_function = runner._kernel_identity
        self.identity_function = runner._environment_identity
        self.expected = self._patch("_environment_identity", return_value=self.identity)
        self.actual = self._patch("_kernel_identity", return_value=dict(self.identity))
        self._patch("KernelManager", return_value=self.manager)
        self._patch("NotebookClient", return_value=self.client)
        self._patch("nbformat.read", return_value=self.notebook)
        self._patch("builtins.print")

    def _patch(self, name, **kwargs):
        target = name if name.startswith("builtins.") else f"run_notebooks.{name}"
        patcher = patch(target, **kwargs)
        self.addCleanup(patcher.stop)
        return patcher.start()

    def test_matching_environment_executes_and_records_portable_versions(self):
        self.assertEqual(runner.execute_notebook(self.path), 1)
        self.client.execute.assert_called_once()
        self.assertEqual(
            self.notebook.metadata.execution_environment,
            {"python": "3.12.13", "quantsupport": "0.1.10", "matches_runner": True},
        )
        self.client.kc.stop_channels.assert_called_once()
        self.manager.shutdown_kernel.assert_called_once_with(now=True)

    def test_environment_mismatch_stops_before_notebook_cells(self):
        self.actual.return_value["executable"] = "/base/bin/python"
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            runner.execute_notebook(self.path)
        self.client.execute.assert_not_called()
        self.manager.shutdown_kernel.assert_called_once()

    def test_stale_binding_stops_even_when_runner_and_kernel_match(self):
        self.identity["FixedRateCoupon"] = False
        self.actual.return_value = dict(self.identity)
        with self.assertRaisesRegex(RuntimeError, "missing or stale"):
            runner.execute_notebook(self.path)
        self.client.execute.assert_not_called()

    def test_explicit_kernel_may_differ_from_runner(self):
        self.actual.return_value["executable"] = "/selected/bin/python"
        runner.execute_notebook(self.path, kernel="selected")
        self.client.execute.assert_called_once()
        self.assertFalse(self.notebook.metadata.execution_environment.matches_runner)

    def test_failed_execution_still_closes_channels_and_kernel(self):
        self.client.execute.side_effect = RuntimeError("cell failed")
        with self.assertRaisesRegex(RuntimeError, "cell failed"):
            runner.execute_notebook(self.path)
        self.client.kc.stop_channels.assert_called_once()
        self.manager.shutdown_kernel.assert_called_once()

    def test_environment_change_during_notebook_execution_is_rejected(self):
        changed = {**self.identity, "executable": "/base/bin/python"}
        self.actual.side_effect = [dict(self.identity), changed]
        with self.assertRaisesRegex(RuntimeError, "changed during execution"):
            runner.execute_notebook(self.path)
        self.client.execute.assert_called_once()
        self.assertNotIn("execution_environment", self.notebook.metadata)

    def test_probe_ignores_unrelated_replies_and_stops_channels(self):
        probe = self.manager.blocking_client.return_value
        probe.execute.return_value = "probe"
        probe.get_shell_msg.side_effect = [
            {"parent_header": {"msg_id": "other"}},
            {
                "parent_header": {"msg_id": "probe"},
                "content": {
                    "status": "ok",
                    "user_expressions": {
                        "identity": {
                            "status": "ok",
                            "data": {"text/plain": repr(self.identity)},
                        }
                    },
                },
            },
        ]
        with patch("run_notebooks._environment_identity", self.identity_function):
            self.assertEqual(self.probe_function(self.manager), self.identity)
        probe.stop_channels.assert_called_once()


class LiveKernelEnvironmentTests(TestCase):
    def setUp(self):
        self.directory = Path("artifacts/notebook-runner-tests") / uuid4().hex
        self.directory.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, self.directory)
        self.identity = runner._environment_identity()
        self.kernel_name = f"runner-named-{uuid4().hex}"
        self.jupyter_path = self.directory / "jupyter"
        self._write_spec(
            "python3",
            [sys.executable, "-c", "raise SystemExit('default spec must be overridden')"],
        )
        self._write_spec(
            self.kernel_name,
            [
                sys.executable,
                "-c",
                "import os; os.environ['RUNNER_NAMED_ARGV'] = 'preserved'; "
                "from ipykernel.kernelapp import launch_new_instance; launch_new_instance()",
                "-f",
                "{connection_file}",
            ],
        )
        environment = patch.dict(
            os.environ, {"JUPYTER_PATH": str(self.jupyter_path.resolve())}
        )
        environment.start()
        self.addCleanup(environment.stop)

    def _write_spec(self, name, argv):
        directory = self.jupyter_path / "kernels" / name
        directory.mkdir(parents=True)
        (directory / "kernel.json").write_text(
            json.dumps(
                {"argv": argv, "display_name": name, "language": "python"}
            ),
            encoding="utf-8",
        )

    def _execute(self, *, kernel=None):
        expected = self.identity
        code = (
            "import os, sys, quantsupport as qs\n"
            "from importlib.metadata import version\n"
            f"assert sys.executable == {expected['executable']!r}\n"
            f"assert sys.prefix == {expected['prefix']!r}\n"
            f"assert qs.__file__ == {expected['quantsupport_file']!r}\n"
            f"assert version('quantsupport') == {expected['quantsupport_version']!r}\n"
            "assert hasattr(qs, 'FixedRateCoupon') and hasattr(qs, 'Shock')\n"
            "assert hasattr(qs.Calendar, 'TARGET')\n"
        )
        if kernel:
            code += "assert os.environ['RUNNER_NAMED_ARGV'] == 'preserved'\n"
        source = self.directory / "source.ipynb"
        output = self.directory / "executed.ipynb"
        nbformat.write(
            nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell(code)]),
            source,
        )
        self.assertEqual(
            runner.execute_notebook(source.resolve(), kernel=kernel, output=output),
            1,
        )
        executed = nbformat.read(output, as_version=4)
        self.assertEqual(executed.cells[0].execution_count, 1)
        self.assertFalse(
            any(item.output_type == "error" for item in executed.cells[0].outputs)
        )
        self.assertTrue(executed.metadata.execution_environment.matches_runner)
        self.assertEqual(
            executed.metadata.execution_environment.quantsupport,
            expected["quantsupport_version"],
        )

    def test_real_default_kernel_uses_caller_not_registered_python3_command(self):
        self._execute()

    def test_real_named_kernel_preserves_its_registered_launch_command(self):
        self._execute(kernel=self.kernel_name)


if __name__ == "__main__":
    main()
