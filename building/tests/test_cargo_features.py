#!/usr/bin/env python3
"""Self-check for the UDS_CARGO_FEATURES passthrough of the build scripts.

Run with ``python3 -m unittest discover -s building/tests`` from the crate root.
"""
import importlib.util
import os
import pathlib
import re
import types
import unittest

BUILDING = pathlib.Path(__file__).resolve().parent.parent
ENV_VAR = 'UDS_CARGO_FEATURES'


def _load(script: pathlib.Path) -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(script.stem.replace('-', '_'), script)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CargoFeaturesArgsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.previous = os.environ.pop(ENV_VAR, None)
        self.modules = [
            _load(BUILDING / 'linux' / 'rustbuilder.py'),
            _load(BUILDING / 'macos' / 'build-pkg.py'),
        ]

    def tearDown(self) -> None:
        os.environ.pop(ENV_VAR, None)
        if self.previous is not None:
            os.environ[ENV_VAR] = self.previous

    def test_no_args_when_variable_is_absent(self) -> None:
        for module in self.modules:
            self.assertEqual(module.cargo_features_args(), [], module.__name__)

    def test_no_args_when_variable_is_blank(self) -> None:
        os.environ[ENV_VAR] = '   '
        for module in self.modules:
            self.assertEqual(module.cargo_features_args(), [], module.__name__)

    def test_single_feature_is_forwarded(self) -> None:
        os.environ[ENV_VAR] = 'insecure-tls'
        for module in self.modules:
            self.assertEqual(module.cargo_features_args(), ['--features', 'insecure-tls'], module.__name__)

    def test_several_features_are_forwarded_verbatim(self) -> None:
        os.environ[ENV_VAR] = ' insecure-tls,other '
        for module in self.modules:
            self.assertEqual(
                module.cargo_features_args(), ['--features', 'insecure-tls,other'], module.__name__
            )


class WindowsScriptTest(unittest.TestCase):
    """The PowerShell script cannot be imported, so its composition is checked as text."""

    def setUp(self) -> None:
        self.script = (BUILDING / 'windows' / 'build.ps1').read_text()

    def test_cargo_command_is_built_from_the_variable(self) -> None:
        self.assertIn('$cargoCmd = @("cargo", "build", "--release")', self.script)
        self.assertRegex(
            self.script,
            re.compile(
                r'if \(\$env:UDS_CARGO_FEATURES\) \{ \$cargoCmd \+= @\("--features", \$env:UDS_CARGO_FEATURES\) \}'
            ),
        )

    def test_docker_runs_the_composed_command(self) -> None:
        self.assertIn('udslauncher-builder @cargoCmd', self.script)
        self.assertNotIn('udslauncher-builder cargo build --release', self.script)


if __name__ == '__main__':
    unittest.main()
