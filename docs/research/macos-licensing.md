# macOS dependency licensing review

Reviewed 28 October 2026 against Clay Form's locked dependencies at `12690cd`
and the open-source preparation branch. This is a technical licensing review,
not a legal clearance or an Apple permission grant.

## Finding

No GPL/MIT incompatibility was identified. There are two narrower unresolved
questions: the completeness of historical copyright notices and the status of
bindings generated from Apple SDK declarations. Neither establishes that Clay
Form's macOS build is prohibited. Neither can be dismissed solely because the
crates are widely used.

## What Clay Form actually uses

The native dependency graph contains both versions of the Objective-C bridge:

| Dependency | Why it is present | Declared terms |
| --- | --- | --- |
| `objc2 0.5.2`, `block2 0.5.1`, `objc2-foundation 0.2.2` | `eframe 0.33.3` / `winit 0.30.13`: application windows and native events | MIT |
| `objc2-app-kit 0.2.2` | Windowing through eframe/winit | MIT |
| `objc2 0.6.4`, `objc2-foundation 0.3.2`, `objc2-encode 4.1.0` | `arboard 3.6.1` via egui-winit: clipboard | MIT |
| `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics 0.3.2` | Clipboard's framework wrappers | Zlib OR Apache-2.0 OR MIT |

Reproduce with `cargo tree --locked --target aarch64-apple-darwin -i objc2@0.6.4`
and repeat for `objc2@0.5.2`. Removing clipboard support would not remove the
windowing dependency. Replacing this bridge is therefore not a small licence
cleanup, and does not inherently resolve the legal status of platform APIs.

## Open-source licence versus notice completeness

The Free Software Foundation lists the standard MIT/Expat licence as
[GPL-compatible](https://www.gnu.org/licenses/license-list.html#Expat).
Selecting MIT where offered is compatible with Clay Form's chosen GPLv3 terms,
provided the applicable notices and other obligations are satisfied.

[Issue #23](https://github.com/madsmtm/objc2/issues/23) proposes adding
Apache/Zlib options to older MIT-only code. It is still open; the proposed
relicensing must not be treated as permission already granted.

In [issue #826](https://github.com/madsmtm/objc2/issues/826), upstream added
full licence texts in March 2026. In October the maintainer confirmed that old
versions retain the licences declared in their manifests, but did not confirm
the sufficiency of the newly added copyright notice for other contributors.
The issue's closed status therefore does not clear the later attribution question.

The older release trees contain a Steven Sheldon MIT notice. The
[new MIT text](https://github.com/madsmtm/objc2/blob/ee9a7ada2131f5944b8750428e265c15632f2a19/LICENSE-MIT.txt)
names Mads Marquart in 2026. Preserve available historical notices and the new
notice with source provenance. Do not invent an earlier date or assume the
new file is a complete replacement for every prior contributor's notice.

## Apple SDK terms are a separate question

The [licensing explanation at our framework revision](https://github.com/madsmtm/objc2/blob/7b1abfd750a2cacaea71d6a56ecfb83cb7de560b/LICENSE.md)
explicitly records upstream's uncertainty about redistribution of SDK-derived
bindings. It is a question raised by the maintainer, not a finding by Apple or
a court, and it is not resolved by adding a generic MIT template.

Apple's [Xcode/SDK agreement](https://www.apple.com/legal/sla/docs/xcode.pdf)
distinguishes app distribution from redistribution of Apple's software:
§2.4 allows compliant macOS applications and libraries to be distributed
without a separate agreement, while App Store distribution has additional
requirements. §2.6 says Apple does not acquire ownership of our application.
§2.7 restricts redistribution and derivative works of Apple software, subject
to its stated exceptions. The practical issue is whether particular generated
binding content falls within those restrictions. This review does not establish
that legal classification. Use an approved macOS/Xcode environment and do not
bundle SDKs, system frameworks or Apple artwork in releases.

GPLv3's [system-library exception](https://www.gnu.org/licenses/gpl-faq.en.html#SystemLibraryException)
addresses use of qualifying proprietary OS libraries. It does not independently
grant rights to redistribute someone else's generated wrappers or SDK content.

## Release decision

Continue macOS build validation and retain the dependency's declared licences.
Keep the two unresolved items visible in release review. Obtain a targeted
licensing assessment of SDK-derived binding redistribution and the collected
historical notices before calling the macOS distribution legally cleared.
No maintainer or Apple contact was made during this research.

Switching Clay Form to a permissive licence would not itself cure missing
third-party notices or SDK-derived-content rights. Upgrading dependencies may
improve packaged notices, but requires its own review and runtime validation.
Direct downloads, signing/notarization and Mac App Store distribution are
different release decisions; the initial packaging work targets direct downloads.
