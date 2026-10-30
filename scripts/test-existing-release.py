#!/usr/bin/env python3
"""Check release preflight decisions without contacting GitHub."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

spec = importlib.util.spec_from_file_location(
    "preflight", Path(__file__).with_name("check-existing-release.py")
)
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)


def response(value):
    return io.BytesIO(json.dumps(value).encode())


class ReleasePreflightTests(unittest.TestCase):
    def api_error(self, code=404):
        error = HTTPError("https://api.github.com", code, "Failed", {}, None)
        self.addCleanup(error.close)
        return error

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

    def test_tag_404_with_matching_draft_skips_builds(self):
        with patch.object(preflight, "urlopen", side_effect=[
            self.api_error(),
            io.BytesIO(b'[{"tag_name":"v0.1.0-alpha.1","draft":true}]'),
        ]) as request:
            self.assertFalse(preflight.build_required())
        self.assertEqual(request.call_count, 2)

    def test_tag_404_with_matching_published_release_skips_builds(self):
        with patch.object(preflight, "urlopen", side_effect=[
            self.api_error(),
            response([{"tag_name": "v0.1.0-alpha.1", "draft": False}]),
        ]):
            self.assertFalse(preflight.build_required())

    def test_manual_release_with_draft_in_list_fails_early(self):
        os.environ["GITHUB_EVENT_NAME"] = "workflow_dispatch"
        with patch.object(preflight, "urlopen", side_effect=[
            self.api_error(),
            response([{"tag_name": "v0.1.0-alpha.1", "draft": True}]),
        ]):
            with self.assertRaisesRegex(SystemExit, "Release already exists"):
                preflight.build_required()

    def test_draft_on_later_page_skips_builds_with_authenticated_requests(self):
        os.environ["GITHUB_API_URL"] = "https://github.example/api/v3"
        with patch.object(preflight, "urlopen", side_effect=[
            self.api_error(),
            response([{"tag_name": f"v0.2.0-alpha.{i}"} for i in range(100)]),
            response([{"tag_name": "v0.1.0-alpha.1", "draft": True}]),
        ]) as request:
            self.assertFalse(preflight.build_required())
        base = "https://github.example/api/v3/repos/example/project/releases"
        self.assertEqual([call.args[0].full_url for call in request.call_args_list], [
            base + "/tags/v0.1.0-alpha.1",
            base + "?per_page=100&page=1",
            base + "?per_page=100&page=2",
        ])
        for call in request.call_args_list:
            self.assertEqual(call.args[0].get_header("Authorization"), "Bearer test-token")
            self.assertEqual(call.args[0].get_header("Accept"), "application/vnd.github+json")
            self.assertEqual(call.kwargs["timeout"], 30)

    def test_missing_release_allows_new_tag_and_manual_builds(self):
        for event in ("push", "workflow_dispatch"):
            os.environ["GITHUB_EVENT_NAME"] = event
            with self.subTest(event=event), patch.object(preflight, "urlopen", side_effect=[
                self.api_error(),
                response([{"tag_name": "v0.1.0-alpha.10", "draft": True}]),
            ]):
                self.assertTrue(preflight.build_required())

    def test_absent_release_after_full_page_requires_final_empty_page(self):
        with patch.object(preflight, "urlopen", side_effect=[
            self.api_error(),
            response([{"tag_name": f"v0.2.0-alpha.{i}"} for i in range(100)]),
            response([]),
        ]) as request:
            self.assertTrue(preflight.build_required())
        self.assertEqual(request.call_count, 3)

    def test_empty_release_list_allows_build(self):
        with patch.object(preflight, "urlopen", side_effect=[self.api_error(), response([])]):
            self.assertTrue(preflight.build_required())

    def test_permission_and_server_errors_do_not_allow_builds(self):
        for code in (401, 403, 429, 500):
            with self.subTest(code=code), patch.object(preflight, "urlopen", side_effect=self.api_error(code)):
                with self.assertRaises(HTTPError):
                    preflight.build_required()

    def test_release_list_errors_do_not_allow_builds(self):
        for page in (1, 2):
            for code in (401, 403, 404, 429, 500):
                responses = [self.api_error()]
                if page == 2:
                    responses.append(response([{"tag_name": "v-other"}] * 100))
                responses.append(self.api_error(code))
                with self.subTest(page=page, code=code), patch.object(
                    preflight, "urlopen", side_effect=responses
                ):
                    with self.assertRaises(HTTPError):
                        preflight.build_required()

    def test_network_errors_do_not_allow_builds(self):
        for fallback in (False, True):
            responses = [self.api_error()] if fallback else []
            responses.append(URLError("Connection failed"))
            with self.subTest(fallback=fallback), patch.object(
                preflight, "urlopen", side_effect=responses
            ):
                with self.assertRaises(URLError):
                    preflight.build_required()

    def test_malformed_list_responses_do_not_allow_builds(self):
        for body in (b'not json', b'{}', b'null', b'[{}]', b'[null]', b'[{"tag_name":42}]'):
            with self.subTest(body=body), patch.object(preflight, "urlopen", side_effect=[
                self.api_error(), io.BytesIO(body),
            ]):
                with self.assertRaises(ValueError):
                    preflight.build_required()

    def test_workflow_output_for_draft_absence_and_error(self):
        for result in ("draft", "absent", "error"):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "output"
                os.environ["GITHUB_OUTPUT"] = str(output)
                listed = {
                    "draft": response([{"tag_name": "v0.1.0-alpha.1", "draft": True}]),
                    "absent": response([]),
                    "error": self.api_error(),
                }[result]
                with patch.object(preflight, "urlopen", side_effect=[self.api_error(), listed]):
                    if result == "error":
                        with self.assertRaises(HTTPError):
                            preflight.main()
                        self.assertFalse(output.exists())
                    else:
                        preflight.main()
                        expected = "false" if result == "draft" else "true"
                        self.assertEqual(output.read_text(), f"build_required={expected}\n")

    def test_mismatched_release_response_fails(self):
        with patch.object(preflight, "urlopen", return_value=io.BytesIO(b'{"tag_name":"v0.1.0-alpha.2"}')):
            with self.assertRaisesRegex(ValueError, "does not match"):
                preflight.build_required()


if __name__ == "__main__":
    unittest.main()
