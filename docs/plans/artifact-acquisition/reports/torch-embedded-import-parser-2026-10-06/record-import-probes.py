"""Record both real test subprocesses without substituting their execution."""
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
repo = Path('/workspace/Pumas-Library')
sys.path.insert(0, str(repo / 'torch-server/tests'))
import test_embedded_runtime_imports as tests

calls = []
original = tests.EmbeddedRuntimeImportTests.probe

def record(self, root):
    result = original(self, root)
    calls.append({'test_id': self.id(), 'interpreter': sys.executable,
                  'isolated': True, 'checkout_pythonpath_offered': str(repo / 'torch-server'),
                  'staged_python_files': sorted(str(p.relative_to(root)) for p in root.rglob('*.py')),
                  'returncode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
    return result

suite = unittest.defaultTestLoader.loadTestsFromTestCase(tests.EmbeddedRuntimeImportTests)
with patch.object(tests.EmbeddedRuntimeImportTests, 'probe', record):
    outcome = unittest.TextTestRunner(verbosity=2).run(suite)
assert outcome.wasSuccessful() and len(calls) == 2
assert calls[0]['returncode'] == 0 and 'EMBEDDED_IMAGE_TEXT_IMPORT_CLOSURE_OK' in calls[0]['stdout']
assert calls[1]['returncode'] != 0 and "No module named 'speech_binding'" in calls[1]['stderr']
Path('/workspace/pumas-parser-fix-evidence/actual-import-probes.json').write_text(json.dumps(calls, indent=2) + '\n')
print('Both original test assertions passed after real isolated subprocess execution.')
