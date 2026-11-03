# ADR-003: Ship prebuilt Linux packages with each release

## Status
Accepted. Amends the "Distribution: Source only" row of `CONTEXT.md` §4 for Linux; macOS and Windows stay source only.

## Date
2026-11-04

## Context
Until now every user cloned the repo and built Harneo for their own OS (`BUILDING.md`): Rust, Node and the Tauri system packages, then two builds. That was fine for the maintainer and contributors, but it is the main barrier for people who find Harneo through a launch post (Show HN asks that a project be easy to try) and only want to see whether it fits their workflow.

Constraints:
- **The maintainer builds and tests on Linux only.** macOS and Windows are "untested by the maintainer" (`CONTEXT.md` §5); shipping binaries for them would ship something nobody has run.
- **Signing.** macOS needs a paid Apple Developer account for signing and notarization, or users see Gatekeeper's "damaged" warning; Windows shows SmartScreen without a code-signing certificate. Linux packages need neither.
- **The release gate stays.** A release is a `release_x.y.z` tag whose `check` workflow (`check:full`) passed before the GitHub Release is published (`CLAUDE.md`). Binaries must never come from an unchecked commit.
- **No new trust in third parties at build time** beyond what CI already uses (GitHub-hosted runners, the Rust and Node setup actions).

## Decision
Each published GitHub Release gets a Linux **AppImage** and a **.deb**, plus a `SHA256SUMS` file:
- A `linux-packages` workflow runs when a release is **published**, so it only ever builds a tag whose `check` run already passed. It can also be run by hand for a tag.
- It builds on **Ubuntu 22.04** (an older glibc, so the AppImage runs on more distributions) with Tauri's own bundler (`tauri build --bundles appimage,deb`) and attaches the files to that release with `gh release upload`.
- The app's bundle config (`src-tauri/tauri.conf.json`) carries the targets; building from source keeps `--no-bundle` and is unchanged.
- **macOS and Windows stay source only**, with no installers and no code signing, until someone tests them.

## Alternatives Considered

### Stay source only (status quo)
- Pros: nothing to maintain; no binaries to trust.
- Cons: most people won't build a Tauri app to try one.
- Rejected by the user (2026-11-04).

### Packages for every OS
- Pros: everyone gets a download.
- Cons: unsigned macOS and Windows builds trip OS warnings, and nobody has tested them.
- Rejected: ship only what the maintainer runs.

### `tauri-apps/tauri-action`
- Pros: one step that builds and uploads, and can create the release.
- Cons: another third-party action with write access to releases; it overlaps with the release step `CLAUDE.md` already defines.
- Rejected: `tauri build` plus `gh release upload` does the same with tools CI already has.

### Flatpak or Snap
- Pros: sandboxing, store listings.
- Cons: a store submission and review per release; Harneo needs broad file access (projects, `~/.claude`), which these sandboxes restrict.
- Rejected for now; AppImage and .deb cover most desktops.

## Consequences
- Release notes and the README point Linux users to the release assets, with `sha256sum -c SHA256SUMS`; building from source stays documented for every OS.
- A release is only complete once `linux-packages` has attached its files; the release steps in `CLAUDE.md` include checking that.
- The packages are unsigned: `SHA256SUMS` proves a download is intact, not who built it. The workflow run on the tag is the provenance.
- The AppImage carries its own libraries, so it is much larger than the binary; the .deb relies on the system's WebKitGTK 4.1 and targets Debian and Ubuntu.
