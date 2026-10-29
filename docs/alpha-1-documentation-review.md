# Review the Alpha 1 documentation

[Development](development.md) · [Documentation home](../README.md#documentation)

Use this checklist to review the documentation and packages before the first public Alpha 1 release. Completing it produces a review record; it does not publish a release or change repository visibility.

## Choose the right release instructions

Alpha 1 preparation is pending in [PR #77](https://github.com/dustyghost/clay-form/pull/77). The following links point to its reviewed snapshot at `e6f0b553e9d6cf5579e869327bff574d5275be5d`, so they remain readable before those files reach `main`.

| You need to… | Read |
| --- | --- |
| Understand contribution rights, attribution and bug reports | [Contributor guidance](https://github.com/dustyghost/clay-form/blob/e6f0b553e9d6cf5579e869327bff574d5275be5d/CONTRIBUTING.md) |
| Review platform support and remaining launch decisions | [Open-source readiness](https://github.com/dustyghost/clay-form/blob/e6f0b553e9d6cf5579e869327bff574d5275be5d/docs/open-source-readiness.md) |
| Prepare and inspect an Alpha draft | [Alpha release instructions](https://github.com/dustyghost/clay-form/blob/e6f0b553e9d6cf5579e869327bff574d5275be5d/.github/RELEASING.md) |
| Understand recorded attribution uncertainty | [Licence verification findings](https://github.com/dustyghost/clay-form/blob/e6f0b553e9d6cf5579e869327bff574d5275be5d/docs/research/license-verification.md) |

**Check the branch before running a workflow.** The current [`main` release instructions](../.github/RELEASING.md) describe a Linux workflow that publishes automatically. PR #77 proposes a six-target **Native packages** workflow that creates drafts. Do not use the current workflow as an Alpha draft preview.

After PR #77 is accepted, compare these snapshot instructions with the final merged files. Update this reading route to the accepted documentation and resolve any changed requirements before preparing a release.

## Review the first-time user journey

1. Start from the README with a fresh checkout or an unpacked candidate archive. Follow its installation and first-sculpt links without relying on development knowledge.
2. Check that the instructions distinguish available downloads from proposed packages. Confirm the stated OS, architecture and runtime requirements against the exact candidate.
3. Complete the first sculpt, save it, close the app and reopen the project. Check that screenshots and exact control names match what you see.
4. Follow the links for sculpting, remeshing and recovery. Confirm that destructive-operation warnings and geometry limits remain visible beside the affected steps.
5. Record the candidate commit, package filename, OS, GPU/input device, result and any confusing instruction in the review PR or issue.

The review should leave a reproducible account of the beginner journey and a short list of remaining corrections. A package that builds successfully still needs real desktop, GPU and input testing.

## Review candidate packages and notices

Use the release procedure belonging to the accepted release commit. For the proposed Alpha workflow, review the draft assets before any separate publication decision.

1. Require the final CI and native packaging jobs to pass on the candidate commit.
2. Download each candidate archive and verify its checksum. Compare its contents with the release asset table and check the recorded build identity.
3. Check that packaged documentation, offline notices and corresponding source are present and match the candidate. Follow the source rebuild instructions and record the result.
4. Run the notice verification required by the accepted workflow. Read the recorded distribution decisions as well as the pass/fail result.
5. Check that the release notes state the tested platforms, known limitations and development status. Remove unresolved checklist placeholders only after recording their outcomes.

A successful notice-hash check alone is not legal clearance. Preserve third-party notices, provenance and recorded uncertainty; refer legal questions to the maintainer rather than rewriting them as resolved. The pending Alpha preparation uses GPL-3.0-only; this documentation review does not change its terms or impose an upstream repository-link requirement.

## Record the decisions still needing human review

| Review | Evidence or decision to record |
| --- | --- |
| Real platforms and input | Launch, rendering, save/overwrite/recovery, pen pressure, non-ASCII paths and external-editor discovery on the claimed platforms |
| Clean-machine requirements | Required runtime libraries and drivers; include Windows Visual C++ runtime and Linux glibc requirements from the accepted readiness document |
| Download trust | Windows signing and macOS Developer ID/notarisation decisions; check actual OS prompts without asking users to disable security globally |
| Security preparation | Private vulnerability reporting, repository security controls, dependency-advisory scanning and a response process |
| Attribution and source | Required notices, corresponding-source availability and any updated authoritative evidence affecting recorded decisions |
| Final public review | History, artwork, author details, support expectations and release-note accuracy |

Keep unresolved items in the release review record. Publication and repository visibility require their own maintainer decisions after the review; neither is a documentation-editing step.
