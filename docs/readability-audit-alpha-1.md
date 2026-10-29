# Alpha 1 documentation readability audit

[Documentation home](../README.md#documentation) · [Writing standards](documentation-style.md)

This audit identifies the changes that will help a new user install Clay Form, finish a first sculpt and find the next task. It also separates user instructions from the technical contracts contributors need.

Reviewed on 29 October 2026 for [issue #79](https://github.com/dustyghost/clay-form/issues/79). The baseline is `main` at `7728b68d5031bdd776ea995fe4067fa3aac66fd6`. The pending Alpha 1 preparation in [PR #77](https://github.com/dustyghost/clay-form/pull/77), at `e6f0b553e9d6cf5579e869327bff574d5275be5d`, was read for release context only.

## What already works

The documentation home has useful topic tables, and most guides link back to it. The first-sculpt guide already offers a small, achievable project, recognisable results, recovery advice and a useful next-task table.

Feature guides contain valuable examples, screenshots and explicit geometry limits. The architecture overview has an ownership table and preserves links to detailed contracts. Keep these strengths when rewriting.

## Priorities and ownership

P0 items affect the first session or a choice that can discard work. P1 items improve subsequent learning and contributor navigation. P2 items need a human or a later review.

| Priority | File or section | Finding and concrete improvement | Workstream |
| --- | --- | --- | --- |
| P0 | [README introduction, Run and Scope](../README.md) | The first sentence leads with Rust, eframe/egui and mesh vertices. Explain what readers can make first. Separate source setup from using a packaged build, and split the 96-word Scope paragraph into capabilities and limits. Keep current availability distinct from pending Alpha 1 packages. | [#80](https://github.com/dustyghost/clay-form/issues/80) |
| P0 | Installation entry point | There is no dedicated installation page on the baseline. Add prerequisites, a numbered source-build route, expected startup result and practical troubleshooting. Do not claim that pending six-platform Alpha 1 packages are already published. Link to the first project. | #80 |
| P0 | [First sculpt](guide/getting-started.md), soften and save tasks | Preserve the existing project and anchor aliases. Number the actions for smoothing and saving, keep recovery advice nearby, and use UK spelling in prose while retaining exact UI labels. Make opening the app a clear prerequisite. | #80 |
| P0 | [Sculpting](guide/sculpting.md), alphas and adaptive sculpting | The alpha explanation is about 141 words in one paragraph. Put choosing an alpha into steps and library actions into a table. Separate adaptive setup, quad prerequisites and mask protection; retain the subdivision-loss warning, closed-surface requirement and work limits. | [#81](https://github.com/dustyghost/clay-form/issues/81) |
| P0 | [Mesh management](guide/modeling.md#mesh-management), Dynamic Mesh | Algorithm descriptions interrupt the remesh task. Start with when to remesh, then numbered steps and a Fast/Optimized comparison. Keep mask clearing, stored-level loss, closed-geometry requirements, partial-fit warnings and Undo guidance beside the action. Keep attribution and the sampled 0.4-voxel bound accurate. | #81 |
| P0 | [Texture painting](guide/textures.md), UV island editing and Paint an image | A 191-word paragraph combines selection, transforms, packing and warnings; a 172-word paragraph combines map limitations and image creation. Make these separate tasks, explain UV islands before controls and use option tables. Preserve topology invalidation, shared-image effects, precision, memory and export limits. | #81 |
| P1 | [Sculpting](guide/sculpting.md), paths, Relax and history | Separate the path setup from its interaction rules. Explain Relax as redistributing existing polygons before its session-reference details. Present history limits and eviction behaviour as a compact table, retaining the distinction between history and saved projects. | #81 |
| P1 | [Materials](guide/materials.md), Shading and Shared surfaces and layers | Lighting paragraphs reach 110–138 words and the layer paragraph 129 words. Give the light widget, scene lights and layer creation distinct steps. Keep shared-material edits separate from choosing a colour for future strokes. | #81 |
| P1 | [Selection](guide/selection.md), hold gestures | Explain the everyday hold-and-draw task before optional rebinding and gesture precedence. Preserve the precedence rules as a reference table rather than removing them. | #81 |
| P1 | [Paths](guide/paths.md), [navigation](guide/navigation.md), [files](guide/files.md), [edges](guide/edges.md), [workspace](guide/workspace.md), [references](guide/references.md) | These already contain tasks and screenshots. Add or refine concise task navigation where missing, split combined actions, and check expected outcomes. Avoid a blanket rewrite of already clear examples. | #81 |
| P1 | [Development](development.md) | Add a clear setup-to-first-run path and distinguish focused local checks from the full CI suite. Make screenshot generation and project review separate tasks. Keep version 1 compatibility and existing test policy. | [#82](https://github.com/dustyghost/clay-form/issues/82) |
| P1 | [Architecture](architecture.md) and [editing contracts](architecture/editing.md) | Keep the overview and existing compatibility anchors. Split the roughly 210-word Smooth algorithm paragraph into purpose, method and safeguards without dropping equations, thresholds or ownership details. Technical depth belongs here. | #82 |
| P1 | [Release instructions](../.github/RELEASING.md) and pending Alpha 1 guidance | Baseline instructions publish a Linux release; PR #77 changes this to six native targets and draft-only creation. Document the distinction explicitly and provide a scannable preflight and verification outcome. Do not copy pending release claims onto baseline workflow instructions or edit PR #77. | #82 |
| P2 | Screenshots and real-platform checks | Retain useful captures. First-launch/package extraction images may help after the Alpha 1 packaging UI and platform checks settle. A matched Fast/Optimized remesh comparison may help users choose, if it shows a repeatable difference at readable size. | Human review |

These file boundaries allow independent PRs. The audit owns this page; beginner work owns the README, first-sculpt guide and installation page; feature work owns the remaining `docs/guide/` pages; developer work owns contributor, architecture and release instructions. Coordinate shared navigation links before adding them.

## Examples of the intended changes

These examples illustrate wording and structure; the implementation PRs must check each statement against the relevant controls and constraints.

| Before | Suggested approach |
| --- | --- |
| “A Rust desktop sculpting prototype built with eframe/egui.” | “Clay Form is a desktop sculpting prototype. Start with a sphere and shape it with a mouse or pen.” Put the implementation framework in contributor guidance. |
| “Choose Quads or Triangles… For Quads, choose Fast quads… or Optimized quads…” followed by algorithm paragraphs | Explain that remeshing rebuilds the surface, give the steps, then compare speed and shape/layout trade-offs in a table. Keep all loss and geometry warnings visible. |
| “Click an island… Choose Move, Rotate, or Scale… Auto arrange all…” in one paragraph | Use separate tasks for selecting an island, transforming it and arranging islands. After each task, describe the visible result and cancellation or Undo behaviour. |
| Several sentences mixing history budget, eviction, persistence and diagnostics | Use a table for default/count/memory limits, then short paragraphs for eviction and saved-project behaviour. Link to diagnostic settings once. |

## Duplication and terminology

Explain the first task once and link to its authoritative guide. The README should introduce and route; it does not need renderer, picking or file-format internals. The first project needs only the controls used to finish that project.

Keep user-facing material instructions in `docs/guide/materials.md` and the material-channel contract in `docs/materials.md`. Cross-link them rather than copying storage and export implementation details. Likewise, preserve adaptive and remesh limits in developer contracts while keeping the corresponding user warnings in their task guides.

Use UK English in prose, including “colour”, “centre”, “modelling”, “behaviour” and “optimised”. Preserve exact UI labels, file paths, command names and existing heading anchors even when their spelling differs. Introduce terms such as UV island, topology and subdivision in the task where readers first need them.

## Link, anchor and formatting baseline

A local scan of all repository Markdown checked **438 relative link targets**, including image paths and heading fragments, with **zero missing paths or heading anchors**. The check included explicit HTML anchor aliases and duplicate heading suffixes. External URLs were excluded; network availability is not a readability check.

The scan extracted Markdown inline links outside fenced code, resolved paths relative to each source page, and compared fragments with GitHub-style heading slugs and explicit `<a id>` anchors. It is a static check, so review unusual Markdown constructs in rendered previews as well.

Keep the explicit old step anchors in the first-sculpt guide and the compatibility headings/anchors in `docs/architecture.md`. Re-run the relative-link check on each workstream and on the combined result. Also check `git diff --check`, heading hierarchy, table structure, fenced commands and a rendered preview of changed tasks.

## Final first-time-user review

1. Start at the README and find the correct installation route without understanding Rust internals.
2. Follow that route to an open app, with platform and Alpha limitations stated before the affected action.
3. Finish the moon exercise using the text alone, then save and reopen the `.clay` project.
4. Find navigation, Undo, recovery and the next sculpting task without reading a whole reference page.
5. Find the remesh warnings before trying the operation, and understand how to inspect or undo the result.

The documentation review can check the written route and links. Real desktop, GPU, tablet and package-install tests still need recorded human results; do not treat clearer instructions as evidence that those checks passed.

## Boundaries and remaining review

Historical validation records, legal notices, third-party attribution, the GPLv3 licence, source code and release configuration are outside this rewrite. Leave research evidence intact unless a clearly separate correction is required. Avoid introducing references to competing sculpting applications.

Before public Alpha 1, a maintainer should reconcile pending package instructions with the accepted release workflow, confirm download names and tested platforms, review signing/notarisation guidance, and complete the existing legal/readiness decisions. Clearer wording must not turn a recorded distribution decision into a claim of legal clearance.

Each rewrite PR should reference its issue, include a concrete before/after example and record link/format checks. Keep the PRs unmerged and leave publication and repository visibility unchanged.
