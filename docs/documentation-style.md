# Documentation style

[Documentation home](../README.md#documentation)

Help someone finish a task with Clay Form open beside the guide. Give them a reason to try it, clear steps and a result they can recognise.

## Organize around tasks

- **Give beginners a complete first project.** Keep [Make your first sculpt](guide/getting-started.md) short, achievable and satisfying.
- **Make the next action easy to find.** Start topic guides with a one-sentence purpose and links to their main tasks. Use searchable headings such as “Restore a copy” or “Adjust protection,” with one question per section.
- **Put the task first.** Give numbered steps before detailed options. Use tables for settings, shortcuts and comparisons.
- **Keep reading light.** Use short paragraphs. Explain a limitation where it changes the reader's next action.
- **Keep developer detail elsewhere.** Implementation details, historical changes and test coverage belong in developer docs or the roadmap.
- **Protect links.** Preserve heading anchors where practical; check incoming links when renaming a heading.

Useful references: [Diátaxis on practical instructions](https://diataxis.fr/how-to-guides/), [Figma's task-based help](https://help.figma.com/hc/en-us/articles/360056440594-Create-and-use-variants) and [Google's heading guidance](https://developers.google.com/style/headings).

## Make every screenshot answer a question

Show where to act, which settings matter or what success looks like. Use real app captures of simple practice scenes.

| Check | What to do |
| --- | --- |
| Framing | Focus on the panel for settings; include the viewport when the spatial result matters. |
| Before/after | Keep the camera and framing consistent. Explain intentional view changes. |
| Placement | Put images beside their steps, with concise alt text and a caption saying what to notice. |
| Readability | Keep UI text readable at normal page width. Use side-by-side images only when the important details stay legible. |
| Clean captures | Exclude unrelated windows, clipped controls, placeholder artwork and personal paths. |
| Repeatability | Supply small example assets when they help readers repeat the task. |

Generate captures with [the screenshot script](development.md#update-guide-screenshots), inspect them and update the [inventory](images/README.md). Essential steps must make sense without the image. See [Google's image guidance](https://developers.google.com/style/images) for more examples.
