# markdone Blueprint

## System Overview

markdone provides `t`, a zsh command for personal todo lists kept as Markdown files, one per list
in one folder, each with the sections `## Today`, `## Later` and `## Done`. The command is a single script; an optional zsh
file adds fzf-based Tab completion on top of it. Usage and setup are in [`README.md`](./README.md);
rules for changing the code are in [`AGENTS.md`](./AGENTS.md).

The only coupling between the two components is the output of `t commands` and
`t complete <command>`: `bin/t` prints one tab-separated line per command or per item, and
`shell/t.zsh` shows those lines in fzf.

## Subdirectories & System Boundaries

- bin: [`./bin/EGM.md`](./bin/EGM.md) - `t`, the whole command: reads, tidies and rewrites the
  list, archives old Done items, links Jira keys.
- shell: [`./shell/EGM.md`](./shell/EGM.md) - `t.zsh`, Tab completion for interactive zsh, fed by
  `t commands` and `t complete <command>`.
- test: `run.zsh`, the test suite (no contract file; test-only directory).
- `install` (repository root, no contract file): links `bin/t` into `~/.local/bin` (or `--bin`)
  and appends a marked `source .../shell/t.zsh` line to `${ZDOTDIR:-$HOME}/.zshrc` when fzf is
  installed; `--uninstall` removes exactly those. It never replaces an existing `t`, never touches
  the list, and only warns about a `t` alias or function in `~/.zshrc`.

## Global Invariants

- **Several lists, one folder.** Every `.md` file in the folder of `TODO_FILE` (the default list)
  is a list; a command works on one, chosen by `@<list>`, `TODO_LIST`, `t use` or the default.
  Each list has its own lock, backup, checksum and archives (`Archive/<list> <year>.md`).
- **File format.** The list keeps `## Today`, `## Later` and `## Done`, with `## Done` last. Open
  items are `- [ ] <text>`, done items are `- [x] YYYY-MM-DD <text>`. All other lines are left as
  they are.
- **No silent failures.** When a step fails, the list is left unchanged and `t` either stops
  (`die`) or warns and skips that step (`warn`).
- **One run at a time.** Each run that reads a list holds that list's lock until it exits, so
  concurrent runs from people and agents can't drop each other's changes. Changing the set of lists
  also holds the folder's lock. `t pick` releases it
  while fzf is open and locks again before changing anything.
- **Never lose an item.** Archives are appended before the list is rewritten, so a failure in
  between can duplicate an archived item but not drop one.
- **Configuration only through environment variables.** `TODO_FILE`, `TODO_LIST`,
  `TODO_JIRA_KEYS` and `TODO_JIRA_URL`; nothing environment-specific is hardcoded. The current list
  chosen with `t use` is state, kept in the lists folder, not configuration. `t init` can write `TODO_FILE` to
  `~/.zshenv` for you, but `t` reads no config file of its own.
- **`t commands` and `t complete` are the completion contract.** Their line format
  (`<insert text>\t<display text>`) is shared by `bin/t` and `shell/t.zsh`.
- **Portability.** zsh, POSIX awk and perl on macOS and Linux; no GNU- or BSD-only tool flags.
- **Tests never touch a real list.** `test/run.zsh` sets `TODO_FILE` to throwaway files and
  points `HOME` and `ZDOTDIR` at empty temporary directories, so no test can reach the real home
  (Obsidian's vaults, `~/.zshenv`, `~/.zshrc`, `~/.local/bin`).

## Global Side Effects

- The list file at `$TODO_FILE` (default `~/todo/Todo.md`) is rewritten in place.
- `Archive/<list> <year>.md` files are created and appended to in the lists folder; old
  `Archive <year>.md` files are moved there.
- `t lists add|rm|rename` create, delete and rename list files; `t use` writes `.current` in the
  lists folder. Both hold a hidden `.lists.lock` there.
- Sourcing `shell/t.zsh` rebinds Tab in the interactive zsh session.
- `./install` creates a symlink `~/.local/bin/t` and appends two lines to `~/.zshrc`; only when run.
- A hidden backup, `.<list name>.bak`, next to the list holds the list as it was before `t`'s last
  write. `t undo` restores it, but only while the list still matches the checksum of `t`'s last
  write, kept in `.<list name>.sum`.
- A hidden, empty lock file, `.<list name>.lock`, sits next to the list; `t` holds a lock on it
  while it runs.
- `t init` may add a marked `TODO_FILE` line to `~/.zshenv`, after asking in a terminal, and reads
  Obsidian's list of vaults.
- No network access.

## System Verification

- Run: `test/run.zsh` (needs `python3` and zsh's `zsh/zpty` and `zsh/zselect` modules; must end
  with `N passed, 0 failed`)
