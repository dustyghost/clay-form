# Working agreements

- Run all GitHub CLI (`gh`) commands with escalated permissions as the logged-in user; do not run `gh` as the sandbox identity.

## Keep iteration fast

- Isolate a reported failure with the smallest useful reproduction before starting expensive builds or broad checks.
- Iterate with focused tests and lightweight visual checks when appearance matters. Geometry checks alone do not establish that a visual artifact is fixed.
- Avoid overlapping Cargo builds or tests that compete for the same build lock.
- Once the patch is stable, run the required full checks and release build once. Repeat only when subsequent changes, failures, or unresolved concerns justify it.
- Keep progress updates brief and informative: state what was learned and what the next check will resolve.

## Story tracking

- Add every new issue/story in `dustyghost/clay-form` to [Clay Form Development](https://github.com/users/dustyghost/projects/6) as part of creating it. Do not leave project membership as a later cleanup step.
- Set new work to **Backlog** unless another status is already justified; move every story in an agreed implementation batch to **In progress** when work starts.
- Verify project membership and status through GitHub after creation or an update. If automatic addition fails or is unavailable, add the issue explicitly before reporting it as tracked.
- Keep stories **In progress** while their PR awaits review or merge; mark them **Done** only after acceptance and merge.
