# Designs

| Design | Date | Author | Status | Components | Description |
| ------ | ---- | ------ | ------ | ---------- | ----------- |
| [Port `t` to Rust](./rust_port.md) | 2026-11-14 | sharpmist | Implemented | `src/`, `tests/`, `test/`, `install`, CI, `README.md`, `AGENTS.md`, EGM contracts | Replace the zsh script with one Rust program that runs on macOS, Linux and Windows. |
| [Install `t` with Homebrew](./homebrew.md) | 2026-11-14 | sharpmist | In progress | `.github/workflows/release.yml`, a Homebrew tap, `README.md` | Add `brew install sharpmist/tap/markdone` from the prebuilt release archives, with Tab completion left for the user to turn on. |
