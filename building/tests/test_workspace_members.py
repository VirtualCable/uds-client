#!/usr/bin/env python3
"""Self-check: a plain ``cargo build`` must not compile the Android crate.

Run with ``python3 -m unittest discover -s building/tests`` from the crate root.
"""
import json
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
ANDROID_PACKAGE = 'uds-android'


def _metadata() -> dict:
    out = subprocess.run(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1'],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)


class WorkspaceMembersTest(unittest.TestCase):
    def setUp(self) -> None:
        self.metadata = _metadata()
        self.names = {p['id']: p['name'] for p in self.metadata['packages']}

    def test_android_is_a_member_but_not_a_default_member(self) -> None:
        members = {self.names[i] for i in self.metadata['workspace_members']}
        defaults = {self.names[i] for i in self.metadata['workspace_default_members']}
        self.assertIn(ANDROID_PACKAGE, members)
        self.assertNotIn(ANDROID_PACKAGE, defaults)

    def test_every_other_member_is_built_by_default(self) -> None:
        members = {self.names[i] for i in self.metadata['workspace_members']}
        defaults = {self.names[i] for i in self.metadata['workspace_default_members']}
        self.assertEqual(defaults, members - {ANDROID_PACKAGE})


if __name__ == '__main__':
    unittest.main()
