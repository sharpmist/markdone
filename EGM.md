# markdone Blueprint

## System Overview

markdone provides `t`, a command for personal todo lists kept as Markdown files, one per list in
one folder, each with the sections `## Today`, `## Later` and `## Done`. The command is one Rust
program that runs on macOS, Linux and Windows; an optional zsh file adds fzf-based Tab completion
on top of it. Usage and setup are in [`README.md`](./README.md); rules for changing the code are
in [`AGENTS.md`](./AGENTS.md); the design of the port from zsh to Rust is in
[`docs/designs/rust_port.md`](./docs/designs/rust_port.md).

The only coupling between the two components is the output of `t commands` and
`t complete <command>`: `t` prints one tab-separated line per command or per item, and
`shell/t.zsh` shows those lines in fzf.

## Subdirectories & System Boundaries

- src: [`./src/EGM.md`](./src/EGM.md) - `t`, the whole command: reads, tidies and rewrites the
  lists, archives old Done items, links Jira keys, reads the settings.
- shell: [`./shell/EGM.md`](./shell/EGM.md) - `t.zsh`, Tab completion for interactive zsh, fed by
  `t commands` and `t complete <command>`.
- tests: the integration tests, run by `cargo test` on all three systems (no contract file).
- test: `run.zsh`, the checks that need zsh or a terminal (no contract file).
- docs/designs: design documents, indexed in `SITEMAP.md` (no contract file).
- `install` (repository root, no contract file; macOS and Linux): builds `t` with
  `cargo build --release --locked` (unless `--no-build`), links `target/release/t` into
  `~/.local/bin` (or `--bin`) and appends a marked `source .../shell/t.zsh` line to
  `${ZDOTDIR:-$HOME}/.zshrc` when fzf is installed; `--uninstall` removes exactly those. It never
  replaces another `t` (a link to 0.3's `bin/t` counts as this one), never touches the lists, and
  only warns about a `t` alias or function in `~/.zshrc`.

## Global Invariants

- **Several lists, one folder.** Every `.md` file in the folder of `TODO_FILE` (the default list)
  is a list; a command works on one, chosen by `@<list>`, `TODO_LIST`, `t use` or the default.
  Each list has its own lock, backup, checksum and archives (`Archive/<list> <year>.md`).
- **File format.** The list keeps `## Today`, `## Later` and `## Done`, with `## Done` last. Open
  items are `- [ ] <text>`, done items are `- [x] YYYY-MM-DD <text>`. All other lines are left as
  they are, line endings included (LF or CRLF).
- **No silent failures.** When a step fails, the list is left unchanged and `t` either stops
  (`t: <message>`, exit 1) or warns and skips that step.
- **One run at a time.** Each run that reads a list holds that list's lock until it exits, so
  concurrent runs from people and agents can't drop each other's changes. Changing the set of lists
  also holds the folder's lock. `t pick` releases it while fzf is open and locks again before
  changing anything.
- **Never lose an item.** Archives are appended before the list is rewritten, so a failure in
  between can duplicate an archived item but not drop one.
- **Configuration through environment variables or one config file.** `TODO_FILE`,
  `TODO_JIRA_KEYS` and `TODO_JIRA_URL` come from the environment, else from
  `$XDG_CONFIG_HOME/markdone/config` (`~/.config/markdone/config`; `%APPDATA%\markdone\config` on
  Windows); `TODO_LIST` only from the environment. Nothing environment-specific is hardcoded. The
  current list chosen with `t use` is state, kept in the lists folder, not configuration.
- **`t commands` and `t complete` are the completion contract.** Their line format
  (`<insert text>\t<display text>`) is shared by `src/complete.rs` and `shell/t.zsh`.
- **Portability.** One Rust program for macOS, Linux and Windows; code specific to one system only
  in the places the design lists. `shell/t.zsh` and `install` need zsh.
- **Tests never touch a real list.** `cargo test` runs `t` with its home directory, config folders
  and lists inside a temporary directory and the environment cleared; `test/run.zsh` points `HOME`,
  `ZDOTDIR`, `XDG_CONFIG_HOME` and `APPDATA` at empty temporary directories. No test can reach the
  real home (Obsidian's vaults, the config file, `~/.zshrc`, `~/.local/bin`).

## Global Side Effects

- The list file at `$TODO_FILE` (default `~/todo/Todo.md`) is rewritten in place.
- `Archive/<list> <year>.md` files are created and appended to in the lists folder; old
  `Archive <year>.md` files are moved there.
- `t lists add|rm|rename` create, delete and rename list files; `t use` writes `.current` in the
  lists folder. Both hold a hidden `.lists.lock` there.
- Sourcing `shell/t.zsh` rebinds Tab in the interactive zsh session.
- `./install` builds `target/release/t`, creates a symlink `~/.local/bin/t` and appends two lines
  to `~/.zshrc`; only when run.
- A hidden backup, `.<list file>.bak`, next to the list holds the list as it was before `t`'s last
  write. `t undo` restores it, but only while the list still matches the checksum of `t`'s last
  write, kept in `.<list file>.sum`.
- A hidden, empty lock file, `.<list file>.lock`, sits next to the list; `t` holds a lock on it
  while it runs.
- `t init` may write a `TODO_FILE` line to the config file, after asking in a terminal, and reads
  Obsidian's list of vaults.
- No network access.

## System Verification

- Run: `cargo test` (all systems), then `test/run.zsh` on macOS or Linux (needs a built
  `target/debug/t`, `python3`, and zsh's `zsh/zpty` and `zsh/zselect` modules). Both must pass.
