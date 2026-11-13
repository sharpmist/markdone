# markdone Blueprint

## System Overview

markdone provides `t`, a zsh command for a personal todo list kept in one Markdown file with the
sections `## Today`, `## Later` and `## Done`. The command is a single script; an optional zsh
file adds fzf-based Tab completion on top of it. Usage and setup are in [`README.md`](./README.md);
rules for changing the code are in [`AGENTS.md`](./AGENTS.md).

The only coupling between the two components is the output of `t commands`: `bin/t` prints one
tab-separated line per command, and `shell/t.zsh` shows those lines in fzf.

## Subdirectories & System Boundaries

- bin: [`./bin/EGM.md`](./bin/EGM.md) - `t`, the whole command: reads, tidies and rewrites the
  list, archives old Done items, links Jira keys.
- shell: [`./shell/EGM.md`](./shell/EGM.md) - `t.zsh`, Tab completion for interactive zsh, fed by
  `t commands`.
- test: `run.zsh`, the test suite (no contract file; test-only directory).

## Global Invariants

- **File format.** The list keeps `## Today`, `## Later` and `## Done`, with `## Done` last. Open
  items are `- [ ] <text>`, done items are `- [x] YYYY-MM-DD <text>`. All other lines are left as
  they are.
- **No silent failures.** When a step fails, the list is left unchanged and `t` either stops
  (`die`) or warns and skips that step (`warn`).
- **One run at a time.** Each run that reads the list holds a lock next to it until it exits, so
  concurrent runs from people and agents can't drop each other's changes. `t pick` releases it
  while fzf is open and locks again before changing anything.
- **Never lose an item.** Archives are appended before the list is rewritten, so a failure in
  between can duplicate an archived item but not drop one.
- **Configuration only through environment variables.** `TODO_FILE`, `TODO_JIRA_KEYS` and
  `TODO_JIRA_URL`; nothing environment-specific is hardcoded.
- **`t commands` is the completion contract.** Its line format
  (`<insert text>\t<display text>`) is shared by `bin/t` and `shell/t.zsh`.
- **Portability.** zsh, POSIX awk and perl on macOS and Linux; no GNU- or BSD-only tool flags.
- **Tests never touch a real list.** `test/run.zsh` sets `TODO_FILE` to throwaway files and
  points `ZDOTDIR` at an empty directory.

## Global Side Effects

- The list file at `$TODO_FILE` (default `~/todo/Todo.md`) is rewritten in place.
- `Archive <year>.md` files are created and appended to in the list's directory.
- Sourcing `shell/t.zsh` rebinds Tab in the interactive zsh session.
- A hidden backup, `.<list name>.bak`, next to the list holds the list as it was before `t`'s last
  write. `t undo` restores it.
- A hidden, empty lock file, `.<list name>.lock`, sits next to the list; `t` holds a lock on it
  while it runs.
- No network access.

## System Verification

- Run: `test/run.zsh` (needs `python3` and zsh's `zsh/zpty` and `zsh/zselect` modules; must end
  with `N passed, 0 failed`)
