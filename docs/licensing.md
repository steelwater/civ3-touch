# Licensing decision

Date: 2026-10-02. Owner: Dan. Status: approved in the task chat; mirrored with the milestone audit to canonical Drive.

## Context

The selected FreeC3 revision has root GNU GPL version 3 text and a README declaring GPL-3.0. Its individual Rust manifests do not provide an additional grant. Civ3Touch is intended to remain an independent public open-source derivative without proprietary Civ III payloads.

## Decision

Dan approved GPL-3.0 and the minimal Java/JNI spike. License Civ3Touch additions as GPL-3.0-only, preserve FreeC3's exact license and notices, and retain an unmodified pinned source baseline. This does not assert an explicit upstream “or later” grant.

## Alternatives considered

A permissive-only license for the combined derivative would not reflect the selected upstream terms. Changing engines is outside this brief. Neither was adopted.

## Consequences

For source distribution, preserve copyright/license notices and the GPL text, identify modifications, and license the covered combined work consistently. For any later binary distribution, provide the corresponding source—including relevant build scripts, Lua rules, and selected dependency source—through a GPL-compliant distribution method. Installation information may also be required for applicable User Products. Decide and verify that release method before distributing binaries; this milestone does not authorize publication.

The Android-target Cargo license inventory and copied notices cover the selected Rust dependency graph, including build dependencies. Declared alternatives allow MIT/Apache-compatible choices; Unicode-3.0 notice requirements also remain preserved. This is not a claim that all future dependencies or distribution channels are automatically compatible. Toolchain/platform licenses remain separate. No proprietary GOG file is covered by GPL.

Primary evidence: exact `vendor/freec3/LICENSE`, upstream README, dependency manifests/license files in the resolved packages, `third-party-notices/`, and GPL sections 4–6. No OpenCiv3 code was copied.

Next action: retain these obligations during Milestone 1; perform a release-specific source/notice/provenance check before any binary publication.
