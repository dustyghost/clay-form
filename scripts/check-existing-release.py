#!/usr/bin/env python3
"""Avoid rebuilding a tag whose release already exists; never replace releases."""
import json
import os
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen


def release_json(url):
    request = Request(
        url,
        headers={
            "Authorization": "Bearer " + os.environ["GH_TOKEN"],
            "Accept": "application/vnd.github+json",
        },
    )
    with urlopen(request, timeout=30) as response:
        return json.load(response)


def release_exists(api, repository, tag):
    releases_url = f"{api}/repos/{repository}/releases"
    try:
        release = release_json(f"{releases_url}/tags/{quote(tag, safe='')}")
    except HTTPError as error:
        if error.code != 404:
            raise
        error.close()
    else:
        if not isinstance(release, dict) or release.get("tag_name") != tag:
            raise ValueError("Release response does not match the requested tag")
        return True

    # The by-tag endpoint can return 404 for a draft. Only an authenticated
    # list with push access can establish that neither draft nor release exists.
    page = 1
    per_page = 100
    while True:
        releases = release_json(f"{releases_url}?per_page={per_page}&page={page}")
        if not isinstance(releases, list) or any(
            not isinstance(release, dict) or not isinstance(release.get("tag_name"), str)
            for release in releases
        ):
            raise ValueError("Invalid release list response")
        if any(release["tag_name"] == tag for release in releases):
            return True
        if len(releases) < per_page:
            return False
        page += 1


def build_required():
    event = os.environ["GITHUB_EVENT_NAME"]
    if event == "pull_request":
        return True
    if event not in {"push", "workflow_dispatch"}:
        raise ValueError(f"Unsupported release event: {event}")

    tag = "v" + os.environ["RELEASE_VERSION"]
    api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    repository = os.environ["GITHUB_REPOSITORY"]
    if not release_exists(api, repository, tag):
        return True
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
