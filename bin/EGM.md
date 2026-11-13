# bin

## Overview

`t` is the whole todo command: one zsh script that reads and rewrites a single Markdown list
(`## Today`, `## Later`, `## Done`), using awk and perl for the text processing. Every command
except `help`, `version`, `commands`, `init` and `complete` for a command without items first
checks that the list exists, validates the Jira settings and tidies the list, then runs.

## Interface

Commands, dispatched by the two `case "$1"` blocks at the bottom of `t`:

| Command                              | Aliases          | Handler                         |
| ------------------------------------ | ---------------- | ------------------------------- |
| `t`, `t list`                        | `ls`, `l`        | `[t:list]`                      |
| `t list --today`, `t list --later`   |                  | `[t:list]` with a section name  |
| `t list --done [--all]`              |                  | `[t:list_done]`                 |
| `t list --json`                     |                  | `[t:list_json]`                 |
| `t add [--later] <text>`             | `a`, `-l`        | `[t:add]`, `[t:insert_item]`    |
| `t done <item>`                      | `d`              | `[t:item]`, `[t:finish]`        |
| `t today <item>`                     | `promote`        | `[t:item]`, `[t:move]`          |
| `t later <item>`                     | `demote`         | `[t:item]`, `[t:move]`          |
| `t rm <item>`                        | `remove`         | `[t:item]`, `[t:remove]`        |
| `t reword <item> <text>`             |                  | `[t:item]`, `[t:reword]`        |
| `t pick`                             | `f`              | `[t:pick]`, `[t:finish]`        |
| `t edit`                             | `e`              | `exec ${=EDITOR:-nvim} "$f"`    |
| `t undo`                             |                  | `[t:undo]`                      |
| `t init`                             |                  | `[t:init]`                      |
| `t help`                             | `-h`, `--help`   | `[t:usage]`                     |
| `t version`                          | `--version`      | `print`                         |
| `t commands`                         |                  | `[t:commands]`                  |
| `t complete <command>`               |                  | `[t:complete_items]`            |

Environment variables read at startup:

- `TODO_FILE`: the list file, default `$HOME/todo/Todo.md` (`[t:f]`).
- `TODO_JIRA_KEYS`, `TODO_JIRA_URL`: Jira linking settings (`[t:JIRA_KEYS]`, `[t:JIRA_URL]`),
  validated by `[t:check_jira]`.
- `T_VERSION` (set in `t`, not read from the environment): the release version. A pushed
  `v<version>` tag must match it; the CI workflow checks this.
- `EDITOR`: used by `t edit`, split on spaces, default `nvim`.

Internal helpers:

- `[t:tidy]`: dates and moves ticked items to `## Done`, archives old Done items, links Jira keys.
- `[t:linkify]`: turns bare Jira keys into Markdown links; `linkify all` processes every line.
- `[t:render]`: shows Markdown links as OSC 8 hyperlinks when stdout is a terminal.
- `[t:save]`: rewrites the list from a temp file.
- `[t:undo]`: restores `$bak`, keeping the current list as the new backup.
- `[t:lock]`: takes the list's lock on `$lockfile`.
- `[t:list_json]`: open items as JSON, built with perl so strings are escaped properly.
- `[t:item]`: resolves an `<item>` argument (number or text) to an item number.
- `[t:ITEM_CMDS]`: the commands that take an `<item>`, with their aliases.
- `[t:AWK_SHOWN]`: awk function `shown(s)`, an item's text with `[text](http...)` links reduced to
  their text, as `[t:render]` shows it; used by `[t:item]` and `[t:complete_items]`.
- `[t:pick]`: `t pick`; shows `t list` in fzf with the lock released, then finds the choice again.
- `[t:days_ago]`: date `n` days ago as `YYYY-MM-DD`, using `zsh/datetime`.
- `[t:warn]`, `[t:die]`: print `t: <message>` to stderr; `die` exits with status 1.

Output contracts:

- `t commands`: one line per command, `<text to insert after "t ">\t<display text>`.
  [`../shell/t.zsh`](../shell/EGM.md) parses it.
- `t complete <command>`: one line per open item `<command>` can take,
  `<text to insert after "t <command> ">\t<t list line>`, links shown as their text. The insert
  is the item number, followed by ` ""` for `reword`. `today`/`promote` offer only Later's items,
  `later`/`demote` only Today's. Nothing, with status 0, for a command not in `[t:ITEM_CMDS]`.
- `t list`: one line per open item, `<n>  <section> <text>` (`printf "%3d  %-6s %s\n"`).
  `[t:pick]` takes the item number from the first field of the line chosen in fzf.
- `t list --done`: `<YYYY-MM-DD>  <text>`, newest first.
- `t list --json`: a JSON array in `t list` order, one object per line:
  `{"n":1,"section":"Today","text":"...","jira":["PROJ-1"]}`. `text` is the item as stored
  (Markdown). `jira` holds the configured Jira keys in it, once each, and is empty when linking is
  off. `[]` when there are no open items. Never rendered for a terminal.
- Success messages: `Added <n> (<section>): <text>`, `Done: <text>`, `Removed: <text>`, `Reworded <n> (<section>): <text>`,
  `Moved to <section> as <n>: <text>`, `Created <file>`,
  `Undid the last change; run t undo again to redo it`.

## Invariants

- **The list must exist.** Only `t init` creates it: every command except `help`, `version`,
  `commands`, `init` and `complete` for a command without items dies when `$f` is not a file (`[t:f]` check before `check_jira`).
- **`init` never overwrites.** `[t:init]` dies when anything, including a dangling symlink, is
  already at `$f`. Otherwise it creates the directory (`mkdir -p`) and writes the three headings.
- **One run at a time.** Every command that reads the list calls `[t:lock]` before `check_jira`
  and `tidy`, and holds the lock until it exits, so reading, tidying and writing are one step.
  `[t:lock]` takes an fcntl lock (`zsystem flock`) on `$lockfile`, an empty `.<list name>.lock`
  next to the list, creating the file if needed. It checks every 0.05 s for up to 10 s, then dies.
- **Unlocking.** The kernel releases the lock when the process ends, however it ends (`die`, a
  signal, `kill -9`), so there are no stale locks. Child processes never hold it. `exec` closes the
  lock's descriptor (`zsystem flock` without `-e`), so the editor `t edit` starts doesn't hold it.
  `[t:pick]` is the one exception to holding the lock until exit: it releases it
  (`zsystem flock -u`) while fzf is open, then locks and tidies again before changing anything.
- **Tidy runs first.** `check_jira` and `tidy` run before the second `case` dispatch, so every
  list command, including `edit`, sees a tidied list. `help`, `version`, `commands`, `init` and
  `complete` for a command not in `[t:ITEM_CMDS]` exit before that; all but `init` never touch
  the list. `complete` for an item command reads, locks and tidies the list like `t list`.
- **Writes go through `save`.** `[t:add]`, `[t:finish]`, `[t:move]` and `[t:tidy]` build the result
  in a `mktemp` file and call `[t:save]` only when the step producing it succeeded. `save` first copies
  the list to `$bak` (`.<list name>.bak` next to it, created with `umask 077`) and dies without
  writing when that fails. It then writes with `cat "$1" >"$f"`, so the file itself (symlink,
  permissions) is kept. The write is not atomic; after a crash during it, `$bak` holds the list as
  it was.
- **Undo.** After each write, `[t:save]` records `cksum` of the list in `$sum`
  (`.<list name>.sum`). `[t:undo]` restores `$bak` only when the list still has that checksum, so
  a change made outside `t` since then is never thrown away, even within the same second (content,
  not mtimes). Without `$sum` it refuses too; if writing `$sum` fails, `save` deletes it and
  warns. Undo restores through `[t:save]`, so a second undo redoes. It runs under the lock but
  before `check_jira` and `tidy`, which would otherwise make a change of their own. A
  `[t:tidy]` that archived items deletes `$bak`, because restoring would archive them twice; undo
  then dies with `nothing to undo`.
- **Startup.** `zmodload zsh/datetime zsh/system` must succeed, or `t` exits 1 before doing
  anything. `zsystem flock -i` needs zsh 5.8 or newer.
- **`list` options.** Only `--today`, `--later`, `--done`, `--all` and `--json` are accepted.
  `--all` needs `--done`; `--done` can't be combined with `--today`, `--later` or `--json`;
  `--today --later` together list everything.
- **Archive before rewriting.** `[t:tidy]` appends to every `Archive <year>.md` first and calls
  `save` only if all of those writes worked; otherwise it warns and leaves the list untidied. A
  new archive file starts with a `# Archive <year>` heading; items are appended sorted, one file
  per year of the item's date.
- **Undated Done items.** `[t:tidy]` gives today's date to any `- [x] ` line without a
  `YYYY-MM-DD` date, in `## Done` as well as outside it.
- **`## Done` must follow ticked items.** In `[t:tidy]`, ticked items outside Done are moved only
  when a `## Done` heading comes after them. If any are left over, awk exits non-zero and the list
  is not changed.
- **Archive cutoff.** A Done item is archived when its date (characters 7-16 of the line) sorts
  below `days_ago 7`. If that date can't be computed, nothing is archived.
- **Item references.** `[t:item]` turns `<item>` into a number. Only digits: used as the number
  as is. Anything else: the arguments joined with spaces, matched case-insensitively and literally
  (awk `index`, not a regex) against each open item's text as `[t:render]` shows it in a
  terminal: `[text](http...)` links count by their text, so a linked Jira key matches but no URL
  inside a link does. Exactly one match is required; with none or several, it dies, listing the
  matches, and the list is unchanged. It runs under the lock, so the number can't change before
  it's used.
- **Item numbers.** `n` counts `- [ ] ` lines from the top of the file, across all sections.
  `[t:list]`, `[t:finish]` and `[t:move]` all count the same way; a section filter in `list` keeps
  the numbers.
- **Item text.** `[t:add]` joins its arguments with spaces (`"$*"`), and rejects empty text and
  text containing a newline.
- **Removing.** `[t:remove]` deletes only the line of open item `n`; done items can't be removed.
  `t undo` restores it.
- **Rewording.** `t reword` takes the item from its first argument only and joins the rest into
  the new text. `[t:reword]` replaces the text of open item `n` in place, with the same checks and
  Jira linking as `t add` (non-empty, one line).
- **Moves.** `[t:move]` dies when item `n` doesn't exist or is already in the target section, and
  removes and re-inserts the item in a single write.
- **Insert position.** `[t:insert_item]` appends at the end of the section, before its trailing
  blank lines, and fails when the section heading is missing.
- **Picking.** `[t:pick]` dies when fzf isn't installed or there are no open items, and exits
  with fzf's status when nothing is chosen. After relocking it finds the chosen item by its exact
  stored text: the item at the chosen number if its text is unchanged, else the only open item
  with that text. If there is none, or several and none at the chosen number, it dies and changes
  nothing.
- **Done items.** `[t:finish]` writes `- [x] YYYY-MM-DD <text>` directly under `## Done` and fails
  when item `n` or the heading doesn't exist.
- **Jira linking is all or nothing.** `[t:check_jira]` turns linking off, with a warning, when a
  key doesn't match `^[A-Z][A-Z0-9_]*$`, when the URL is missing, or when the URL doesn't match
  `http(s|)://?*/`. `[t:linkify]` leaves existing Markdown links, inline code and URLs alone, and
  only links the listed keys.
- **Portability.** `#!/usr/bin/env zsh`, POSIX awk only, no GNU- or BSD-only tool flags; date
  arithmetic goes through `[t:days_ago]`.

## Side Effects

- Rewrites `$TODO_FILE` in place (`[t:save]`), from `add`, `done`, `today`, `later`, `rm`, `reword`, `pick` and
  from `[t:tidy]` on any list command when tidying changes the list.
- Overwrites `$bak` with the current list before every write, and `$sum` with the checksum of
  the new list after it (`[t:save]`); deletes `$bak` after a tidy that archived (`[t:tidy]`).
  `t undo` restores `$bak`.
- Creates or appends to `Archive <year>.md` in the list's directory (`[t:tidy]`); reads them for
  `t list --done --all` (`[t:list_done]`).
- Creates `$TODO_FILE` and its directory (`[t:init]`), only when nothing is there yet.
- Creates temp files with `mktemp` and removes them before returning.
- Runs external programs: `awk`, `date`, `cmp`, `cksum`, `cut`, `sort`, `grep`, `cat`, `mktemp`; `perl`
  when Jira linking is on (`[t:linkify]`) or stdout is a terminal (`[t:render]`); `fzf` for
  `t pick`; `$EDITOR` (via `exec`) for `t edit`.
- Writes OSC 8 escape sequences to stdout when it is a terminal (`[t:render]`).
- Writes warnings and errors to stderr prefixed with `t: `; exits 1 on `[t:die]` and on an unknown
  command (after printing the usage to stderr). `t pick` exits with fzf's status when nothing is
  chosen.
- Creates `$lockfile`, an empty file next to the list, and holds an fcntl lock on it while running.
  The file stays; only the lock is released.
- No network access.

## Verification

- Run: `test/run.zsh` (from the repository root)
