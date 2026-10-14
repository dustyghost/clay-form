# Documentation style

[Documentation home](../README.md#documentation)

Write guides for someone using Clay Form with the app open beside them.

## Organize around tasks

- Give beginners one short end-to-end tutorial: [Make your first sculpt](guide/getting-started.md).
- Start topic guides with a one-sentence purpose and links to their main tasks.
- Use headings such as “Restore a copy” or “Adjust protection.” Keep each section about one question.
- Put numbered steps before detailed options. Use tables for settings, shortcuts, and comparisons.
- Keep paragraphs short. Explain a limitation where it changes the user's next action.
- Keep implementation details, historical changes, and test coverage in developer documents or the roadmap.
- Preserve existing heading links where practical; check incoming links when renaming a heading.

This adapts [Diátaxis's separation of practical instructions from reference/explanation](https://diataxis.fr/how-to-guides/) and the task sections used in [Figma's app help](https://help.figma.com/hc/en-us/articles/360056440594-Create-and-use-variants). Google's [heading guidance](https://developers.google.com/style/headings) provides useful rules for descriptive, consistent heading levels.

## Make every screenshot answer a question

Show where to act, which settings matter, or what the result should look like. Use real app captures with simple practice scenes.

- Capture a focused panel for settings; include the viewport when the spatial result matters.
- For a before/after pair, keep the camera and framing consistent. Explain any intentional view change.
- Place images beside their steps. Add concise alt text and a caption explaining what to notice.
- Keep UI text readable at normal page width. Only use side-by-side images when their important details remain legible.
- Avoid unrelated windows, clipped controls, placeholder artwork, and personal paths.
- Supply small example assets when they help readers repeat the task.

These choices follow [Google's image guidance](https://developers.google.com/style/images). Generate captures with [the screenshot script](development.md#update-guide-screenshots), inspect them, and update the [inventory](images/README.md). Screenshots complement written instructions; essential steps must remain understandable without the image.
