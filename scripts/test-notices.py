"""Check that damaged notices and unresolved release reviews cannot pass verification."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class NoticeVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copytree(ROOT / 'legal', self.root / 'legal')
        for name in json.loads((ROOT / 'legal/inputs.json').read_text()):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        shutil.copyfile(ROOT / 'scripts/check-notices.py', self.root / 'scripts/check-notices.py')

    def run_check(self, *args):
        return subprocess.run([sys.executable, self.root / 'scripts/check-notices.py', *args],
                              capture_output=True, text=True)

    def rehash(self):
        path = self.root / 'legal/inputs.json'
        data = json.loads(path.read_text())
        for name in data:
            data[name] = hashlib.sha256((self.root / name).read_bytes()).hexdigest()
        path.write_text(json.dumps(data))

    def test_modified_generated_notices_fail(self):
        self.assertEqual(self.run_check().returncode, 0)
        for name in ['legal/third-party.json', 'THIRD-PARTY-NOTICES.html']:
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_text('Attribution removed')
                result = self.run_check()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(name, result.stderr)
                path.write_bytes(original)

    def test_modified_additional_notice_fails(self):
        entries = json.loads((self.root / 'legal/additional-notices.json').read_text())
        path = self.root / 'legal/upstream' / entries[0]['file']
        path.write_text('Notice removed')
        self.assertNotEqual(self.run_check().returncode, 0)

    def test_platform_review_blocks_release_even_after_notice_resolution(self):
        (self.root / 'legal/pending-review.json').write_text('{}')
        (self.root / 'legal/review-decisions.json').write_text('{}')
        (self.root / 'legal/release-review.json').write_text(json.dumps({
            'review-fixture': {'status': 'pending'}
        }))
        self.rehash()
        self.assertEqual(self.run_check().returncode, 0)
        result = self.run_check('--release')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('review-fixture', result.stdout)
        (self.root / 'legal/release-review.json').write_text(json.dumps({
            'review-fixture': {'status': 'resolved'}
        }))
        self.rehash()
        self.assertEqual(self.run_check('--release').returncode, 0)


if __name__ == '__main__':
    unittest.main()
