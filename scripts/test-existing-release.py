#!/usr/bin/env python3
"""Check release preflight decisions without contacting GitHub."""
import importlib.util
import io
import os
from pathlib import Path
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

spec = importlib.util.spec_from_file_location(
    "preflight", Path(__file__).with_name("check-existing-release.py")
)
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)


class ReleasePreflightTests(unittest.TestCase):
    def setUp(self):
        self.environment = patch.dict(os.environ, {
            "GITHUB_EVENT_NAME": "push",
            "RELEASE_VERSION": "0.1.0-alpha.1",
            "GITHUB_REPOSITORY": "example/project",
            "GITHUB_API_URL": "https://api.github.com",
            "GH_TOKEN": "test-token",
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_pull_request_does_not_query_release_api(self):
        os.environ["GITHUB_EVENT_NAME"] = "pull_request"
        with patch.object(preflight, "urlopen") as request:
            self.assertTrue(preflight.build_required())
        request.assert_not_called()

    def test_existing_draft_or_published_tag_skips_builds(self):
        for draft in (True, False):
            with self.subTest(draft=draft), patch.object(preflight, "urlopen", return_value=io.BytesIO(
                ('{"tag_name":"v0.1.0-alpha.1","draft":' + str(draft).lower() + '}').encode()
            )):
                self.assertFalse(preflight.build_required())

    def test_existing_manual_release_fails_early(self):
        os.environ["GITHUB_EVENT_NAME"] = "workflow_dispatch"
        with patch.object(preflight, "urlopen", return_value=io.BytesIO(b'{"tag_name":"v0.1.0-alpha.1"}')):
            with self.assertRaisesRegex(SystemExit, "Release already exists"):
                preflight.build_required()

    def test_missing_release_allows_new_tag_and_manual_builds(self):
        for event in ("push", "workflow_dispatch"):
            os.environ["GITHUB_EVENT_NAME"] = event
            with self.subTest(event=event), patch.object(preflight, "urlopen", side_effect=HTTPError(
                "https://api.github.com", 404, "Not Found", {}, None
            )):
                self.assertTrue(preflight.build_required())

    def test_permission_and_server_errors_do_not_allow_builds(self):
        for code in (401, 403, 429, 500):
            with self.subTest(code=code), patch.object(preflight, "urlopen", side_effect=HTTPError(
                "https://api.github.com", code, "Failed", {}, None
            )):
                with self.assertRaises(HTTPError):
                    preflight.build_required()

    def test_mismatched_release_response_fails(self):
        with patch.object(preflight, "urlopen", return_value=io.BytesIO(b'{"tag_name":"v0.1.0-alpha.2"}')):
            with self.assertRaisesRegex(ValueError, "does not match"):
                preflight.build_required()


if __name__ == "__main__":
    unittest.main()
