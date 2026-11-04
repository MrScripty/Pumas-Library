"""Shared wait for fixture children that hold custody until they are killed."""

import subprocess
import sys
import threading
import time

# A held fixture child never exits by itself. Wait for its positive report
# rather than a fixed interval (a loaded runner can take longer to start it),
# then confirm it keeps holding across a short observation window. The report
# bound only catches a hung child; it is not part of the pass condition.
_REPORT_BOUND_SECONDS = 120
_OBSERVATION_SECONDS = 1


def assert_child_holds_after(test, script, argv, marker):
    """Run `script argv` and assert it reports `marker`, then keeps running."""
    reported = threading.Event()
    output, error = [], []
    with subprocess.Popen(
        [sys.executable, script, *argv],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ) as child:

        def read_output():
            for line in child.stdout:
                output.append(line)
                if marker in line:
                    reported.set()

        readers = [
            threading.Thread(target=read_output, daemon=True),
            threading.Thread(target=lambda: error.extend(child.stderr), daemon=True),
        ]
        for reader in readers:
            reader.start()
        try:
            deadline = time.monotonic() + _REPORT_BOUND_SECONDS
            while not reported.wait(0.1):
                if child.poll() is not None or time.monotonic() > deadline:
                    break
            if reported.is_set():
                time.sleep(_OBSERVATION_SECONDS)
            exited_with = child.poll()
        finally:
            child.kill()
            child.wait()
            for reader in readers:
                reader.join()
    test.assertIn(marker, "".join(output))
    test.assertIsNone(exited_with, "the fixture child released its hold and exited")
    test.assertNotIn("Traceback", "".join(error))
