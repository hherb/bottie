"""Verify parent-exit policies without terminating this test process or claiming Windows host evidence."""

from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch
from cuda_worker import watch_parent


class ParentWatchdogTests(unittest.TestCase):
    """Windows needs a process handle because its parent PID does not change after parent exit."""

    def test_windows_waits_on_an_open_parent_handle_and_closes_it(self):
        """A pointer-sized handle identifies the actual parent rather than a reusable numeric PID."""
        kernel = SimpleNamespace(
            OpenProcess=MagicMock(return_value=0x10000000001),
            WaitForSingleObject=MagicMock(side_effect=[0x102, 0]),
            CloseHandle=MagicMock(),
        )
        with patch("sys.platform", "win32"), patch("ctypes.WinDLL", return_value=kernel, create=True), patch(
            "os._exit", side_effect=SystemExit(1)
        ), self.assertRaises(SystemExit):
            watch_parent(777)
        kernel.OpenProcess.assert_called_once_with(0x00100000, False, 777)
        kernel.CloseHandle.assert_called_once_with(0x10000000001)

    def test_posix_stops_when_its_parent_changes(self):
        """Reparenting ends a worker whose owning app no longer exists."""
        with patch("sys.platform", "linux"), patch("os.getppid", side_effect=[777, 1]), patch(
            "time.sleep"
        ) as sleep, patch("os._exit", side_effect=SystemExit(1)), self.assertRaises(SystemExit):
            watch_parent(777)
        sleep.assert_called_once_with(1)


if __name__ == "__main__":
    unittest.main()
