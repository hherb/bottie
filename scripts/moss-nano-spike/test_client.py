"""Check the real subprocess boundary with small dependency-free worker fixtures."""

from pathlib import Path
import sys
import tempfile
import time
import unittest

from client import WorkerClient


class ClientTests(unittest.TestCase):
    """Prove deadlines and process reaping independently of model performance."""

    def fixture(self, source: str):
        """Create a temporary Python child with deterministic stdio behaviour."""
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "fixture.py"
        path.write_text(source)
        return WorkerClient([sys.executable, str(path)], Path(directory.name) / "stderr.log")

    def test_partial_line_cannot_bypass_deadline(self):
        """An unterminated stdout fragment must not make a blocking readline hang."""
        child = self.fixture("import sys,time\nsys.stdout.write('{');sys.stdout.flush();time.sleep(20)\n")
        try:
            started = time.monotonic()
            with self.assertRaises(TimeoutError):
                child.read_event(0.1)
            self.assertLess(time.monotonic() - started, 1)
        finally:
            child.stop()
        self.assertIsNotNone(child.process.returncode)

    def test_event_then_eof_requires_terminal_completion(self):
        """EOF after a partial run is an error, never a successful utterance."""
        child = self.fixture('print(\'{"event":"audio"}\',flush=True)\n')
        try:
            self.assertEqual(child.read_event(2), {"event": "audio"})
            with self.assertRaises(RuntimeError):
                child.read_event(2)
        finally:
            child.stop()

    def test_oversized_event_is_rejected_and_child_reaped(self):
        """Unbounded output must not accumulate in the supervisor."""
        child = self.fixture("import sys,time\nsys.stdout.write('x'*700000);sys.stdout.flush();time.sleep(20)\n")
        try:
            with self.assertRaises(ValueError):
                child.read_event(2)
        finally:
            child.stop()
        self.assertIsNotNone(child.process.returncode)

    def test_stop_terminates_and_reaps_live_child(self):
        """Cancellation must leave no running or unreaped worker."""
        child = self.fixture('import time\nprint(\'{"event":"ready"}\',flush=True)\ntime.sleep(20)\n')
        self.assertEqual(child.read_event(2)["event"], "ready")
        self.assertLess(child.stop(), 2)
        self.assertIsNotNone(child.process.returncode)

    def test_buffered_events_cannot_bypass_expired_deadline(self):
        """An overall utterance deadline still applies while output is already buffered."""
        child = self.fixture("import time\ntime.sleep(20)\n")
        child.buffer.extend(b'{"event":"audio"}\n')
        try:
            with self.assertRaises(TimeoutError):
                child.read_event(0)
        finally:
            child.stop()


if __name__ == "__main__":
    unittest.main()
