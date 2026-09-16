"""Tests for exact runtime-file package ownership."""

from __future__ import annotations

import unittest
from pathlib import Path

from diffusers_runtime_ownership import file_owner


class DiffusersRuntimeOwnershipTests(unittest.TestCase):
    """Protect lexical package ownership from symlink-target aliasing."""

    def test_native_symlink_owner_precedes_its_separately_owned_target(self) -> None:
        """Distinct Debian symlink and target packages do not become ambiguous after resolution."""
        observed = Path("/usr/bin/python")
        resolved = Path("/usr/bin/python3.12")
        native_owners = {
            observed: {"deb:python-is-python3@1"},
            resolved: {"deb:python3.12-minimal@2"},
        }

        self.assertEqual(
            file_owner(observed, resolved, set(), {}, native_owners),
            "deb:python-is-python3@1",
        )


if __name__ == "__main__":
    unittest.main()
