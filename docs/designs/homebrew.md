# Install `t` with Homebrew

*   **Author:** sharpmist
*   **Status:** Proposed
*   **Date:** 2026-11-14

--------------------------------------------------------------------------------

## Overview

`t` is one 656 KB Rust binary. Today it is installed by cloning the repo and running `./install`
(which needs Rust and zsh), by `cargo install --path .`, or by unpacking a release archive by
hand. This design adds `brew install sharpmist/tap/markdone` for macOS and Linux: a Homebrew tap
whose formula downloads the release archives the release workflow already publishes.

## The Problem

- Installing needs a clone and a Rust toolchain, or manual unpacking and placing `t` on `PATH`.
- Tab completion (`shell/t.zsh`) is an fzf widget that has to be `source`d from `~/.zshrc`.
  `./install` adds that line; a package manager must not edit a user's dotfiles.
- There is no Linux arm64 archive, so Homebrew on Linux arm64 would have nothing to download.

## Goals

- `brew install sharpmist/tap/markdone` puts `t` on `PATH` and `brew uninstall` removes it.
- The completion script is installed and the user is told, once, how to turn it on.
- A new release reaches the tap with a small, reviewable change.
- No change to `t`'s behaviour, files or commands.

## Non-Goals

- homebrew-core. It has notability requirements; revisit once the project has users.
- Bottles or building from source. The formula uses the prebuilt archives.
- Windows packages (winget, scoop).
- Editing `~/.zshrc` from the formula.

## Open Questions

None left. Decided:

- Linux arm64 matters: the release workflow builds it.
- The repository owner creates the `sharpmist/homebrew-tap` repository (and, later, the token
  secret for automated bumps).

## Technical Plan

### Completion

The formula installs `shell/t.zsh` to `share/markdone/t.zsh` and prints a caveat:

```
source "$(brew --prefix)/share/markdone/t.zsh"   # in ~/.zshrc, after fzf's key bindings
```

A real `_t` completion file is rejected: it would lose the fzf menu `t.zsh` provides.

### Release workflow (`.github/workflows/release.yml`)

It already builds `t-<version>-<target>.tar.gz` for `aarch64-apple-darwin`,
`x86_64-apple-darwin` and `x86_64-unknown-linux-musl` (and a Windows zip), each holding `t`,
LICENSE, README.md and `shell/t.zsh`, and publishes them with `SHA256SUMS`. Change:

- add `aarch64-unknown-linux-musl`, built on the `ubuntu-24.04-arm` runner.

### Tap repository

`sharpmist/homebrew-tap`, with `Formula/markdone.rb`. The formula is named `markdone`, because a
bare `t` is likely to collide. It has:

- `on_macos` and `on_linux` blocks, each split on `Hardware::CPU.arm?`, with the archive URL and
  its sha256 (from `SHA256SUMS`);
- `install`: `bin.install "t"` and `(share/"markdone").install "shell/t.zsh"`;
- `caveats`: the completion line above;
- `test`: runs `t version`, then `add` and `list` with `TODO_FILE` pointing into the test's
  temporary directory.

### Version bumps

By hand for the first release: edit the version, URLs and hashes in the formula, open a PR on the
tap. Once that works, add a last job to `release.yml` that opens the PR (it needs a token scoped to
the tap, which the owner adds as a secret).

### Documentation

README: the Install section gets the `brew install` line and the completion path. `AGENTS.md` and
`EGM.md` mention the tap and the release assets it depends on. The README keeps saying the
binaries aren't signed.

## Alternatives Considered

- **Build from source in the formula** (`cargo install`): users wait for a compile and need Rust
  as a build dependency, for a program whose archive is a few hundred KB.
- **homebrew-core**: see Non-Goals.
- **A `_t` completion file**: see Completion.
- **Have the formula edit `~/.zshrc`**: not something a package manager should do.

## Non-Functional Requirements

- **Size:** the archive is roughly 250–350 KB (an estimate: the binary is 656 KB); installed size
  is under 1 MB.
- **Integrity:** every download is checked against a sha256 in the formula.
- **Safety:** `brew uninstall` leaves `~/.zshrc` and every todo list untouched.

## Verification

- A pull request that changes `release.yml` runs the dry run; every matrix build packs.
- A test tag on a branch or fork publishes the archives and `SHA256SUMS`.
- `brew install --build-from-source ./Formula/markdone.rb`, `brew test markdone` and
  `brew audit --strict --new markdone` pass.
- In a sandbox (throwaway `HOME`, `XDG_CONFIG_HOME`, `ZDOTDIR` and `TODO_FILE`), source the
  installed `t.zsh` and check Tab completion; `test/run.zsh` passes against the installed binary.
