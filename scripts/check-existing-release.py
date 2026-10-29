#!/usr/bin/env python3
"""Avoid rebuilding a tag whose release already exists; never replace releases."""
import json
import os
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen


def build_required():
    event = os.environ["GITHUB_EVENT_NAME"]
    if event == "pull_request":
        return True
    if event not in {"push", "workflow_dispatch"}:
        raise ValueError(f"Unsupported release event: {event}")

    tag = "v" + os.environ["RELEASE_VERSION"]
    api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    repository = os.environ["GITHUB_REPOSITORY"]
    request = Request(
        f"{api}/repos/{repository}/releases/tags/{quote(tag, safe='')}",
        headers={
            "Authorization": "Bearer " + os.environ["GH_TOKEN"],
            "Accept": "application/vnd.github+json",
        },
    )
    try:
        with urlopen(request, timeout=30) as response:
            release = json.load(response)
    except HTTPError as error:
        if error.code == 404:
            return True
        raise

    if release.get("tag_name") != tag:
        raise ValueError("Release response does not match the requested tag")
    if event == "workflow_dispatch":
        raise SystemExit("Release already exists; review it before attempting replacement.")
    print(f"Release {tag} already exists; skipping duplicate tag-triggered builds.")
    return False


def main():
    required = build_required()
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"build_required={str(required).lower()}\n")


if __name__ == "__main__":
    main()
