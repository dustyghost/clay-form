# Working agreements

- Run all GitHub CLI (`gh`) commands with escalated permissions as the logged-in user; do not run `gh` as the sandbox identity.

## Keep iteration fast

- Isolate a reported failure with the smallest useful reproduction before starting expensive builds or broad checks.
- During iteration, run only the focused tests relevant to the code or behavior being changed. Use lightweight visual checks when appearance matters; geometry checks alone do not establish that a visual artifact is fixed.
- Avoid overlapping Cargo builds or tests that compete for the same build lock.
- Let PR CI run the full test suite, formatting, lint, and configured GPU regression checks. Do not duplicate the full checks locally before opening or updating a PR; fix CI failures and require CI to pass on the final commit before merging.
- Leave release builds to the release workflow by default. Run broader checks or a release build locally only when explicitly requested or needed to investigate a specific failure or validate behavior that focused checks and CI do not cover.
- Keep progress updates brief and informative: state what was learned and what the next check will resolve.

## Next work batch: trim redundant CI screenshot work

- Agreed on 24 October 2026: handle this cleanup in the next work batch; leave PR #55's test suite unchanged for now.
- Review moving `clay_buildup_offscreen_preview` and `detail_groove_offscreen_preview` out of routine PR CI into manual visual review. They render images and validate geometry but do not automatically compare visual correctness. Preserve useful capture tools and verify remaining coverage before removing CI invocations.
- Extract the Inflate screenshot sequence from `mesh_refinement_offscreen_review` into a manual capture test; retain the focused Inflate geometry regression.
- Keep core GPU checks for pixels, depth and clipping, and interactive checks that click controls. The mesh-refinement interaction test caught an inaccessible Apply button.
- Keep the distinct relaxation regressions for groove preservation, Inflate cleanup, redistribution beyond the original neighbourhood, and boundaries/masks/nearby sheets.
- Export-review tools and the load profiler are already excluded from routine CI; deleting them would not reduce CI runtime.

## Story tracking

- Add every new issue/story in `dustyghost/clay-form` to [Clay Form Development](https://github.com/users/dustyghost/projects/6) as part of creating it. Do not leave project membership as a later cleanup step.
- Set new work to **Backlog** unless another status is already justified; move every story in an agreed implementation batch to **In progress** when work starts.
- Verify project membership and status through GitHub after creation or an update. If automatic addition fails or is unavailable, add the issue explicitly before reporting it as tracked.
- Keep stories **In progress** while their PR awaits review or merge; mark them **Done** only after acceptance and merge.
