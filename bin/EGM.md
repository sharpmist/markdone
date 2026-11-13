# bin

## Overview

`t` is the whole todo command: one zsh script that reads and rewrites Markdown lists (`## Today`,
`## Later`, `## Done`), one file per list in one folder, using awk and perl for the text
processing. A command works on one list, chosen by `@<list>`, `TODO_LIST`, `t use` or the default.
Every command except `help`, `version`, `commands`, `init`, `lists`, `use` and `complete` without
items to offer first checks that the list exists, validates the Jira settings and tidies the
list, then runs.

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
| `t move <item> @<list>`              | `mv`             | `[t:item]`, `[t:move_to]`       |
| `t pick`                             | `f`              | `[t:pick]`, `[t:finish]`        |
| `t edit`                             | `e`              | `exec ${=EDITOR:-nvim} "$f"`    |
| `t undo`                             |                  | `[t:undo]`                      |
| `t init [<path> \| --obsidian]`      |                  | `[t:init]`, `[t:pick_vault]`    |
| `t help`                             | `-h`, `--help`   | `[t:usage]`                     |
| `t version`                          | `--version`      | `print`                         |
| `t commands`                         |                  | `[t:commands]`                  |
| `t lists [--json]`                   |                  | `[t:lists_cmd]`                 |
| `t lists add <name>`                 |                  | `[t:lists_add]`                 |
| `t lists rm [--force] <name>`        | `remove`         | `[t:lists_rm]`                  |
| `t lists rename <old> <new>`         |                  | `[t:lists_rename]`              |
| `t use [<list>]`                     |                  | `[t:use_cmd]`                   |
| `t @<list> <command>`                |                  | `[t:find_list]`, then `<command>` |
| `t complete <words>`                 |                  | `[t:complete_cmd]`, `[t:complete_items]` |

Environment variables read at startup:

- `TODO_FILE`: the default list, default `$HOME/todo/Todo.md` (`[t:deff]`). Its folder is the
  lists folder (`[t:ldir]`) and its name without the extension the default list's name
  (`[t:dname]`).
- `TODO_LIST`: the list to use when a command has no `@<list>`, overriding `t use`
  (`[t:current_name]`).
- `TODO_JIRA_KEYS`, `TODO_JIRA_URL`: Jira linking settings (`[t:JIRA_KEYS]`, `[t:JIRA_URL]`),
  validated by `[t:check_jira]`.
- `T_VERSION` (set in `t`, not read from the environment): the release version. A pushed
  `v<version>` tag must match it; the CI workflow checks this.
- `EDITOR`: used by `t edit`, split on spaces, default `nvim`.

Internal helpers:

- `[t:set_list]`: makes a list the one this run works on, setting `$name`, `$f`, `$lockfile`,
  `$bak` and `$sum`.
- `[t:list_names]`, `[t:find_list]`, `[t:current_name]`, `[t:check_name]`: the lists, finding one
  by name (exact, else the only case-insensitive match), the list a command without `@<list>`
  uses, and which names are allowed.
- `[t:dir_lock]`: takes the lists folder's lock on `$dirlock` (`.lists.lock`).
- `[t:migrate_archives]`: moves archives from before there were several lists into `$adir`.
- `[t:list_choices]`, `[t:complete_cmd]`: what Tab offers after `t <words>`.
- `[t:open_count]`, `[t:js]`: a list file's open items; a JSON string.
- `[t:move_to]`: moves an open item to another list, both already locked.
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
- `[t:obsidian_vaults]`: the vault directories in Obsidian's `obsidian.json` (macOS, Linux,
  Flatpak and Snap locations), most recently opened first, read with perl's core `JSON::PP`.
- `[t:pick_vault]`: chooses a vault for `t init`, asking only when stdin is a terminal.
- `[t:remember_list]`, `[t:zshenv_line]`: the marked `TODO_FILE` line in `~/.zshenv` for `t init`.
- `[t:ask]`: prints a question and reads a one-line answer; only called when stdin is a terminal.
- `[t:pick]`: `t pick`; shows `t list` in fzf with the lock released, then finds the choice again.
- `[t:days_ago]`: date `n` days ago as `YYYY-MM-DD`, using `zsh/datetime`.
- `[t:warn]`, `[t:die]`: print `t: <message>` to stderr; `die` exits with status 1.

Output contracts:

- `t commands`: one line per command, `<text to insert after "t ">\t<display text>`.
  [`../shell/t.zsh`](../shell/EGM.md) parses it.
- `t complete <words>`: what Tab offers after `t <words> `, one line per choice,
  `<text to insert>\t<display text>`. `@`: the lists as `@<name> `. `@<list>` alone: the commands
  (`t commands`). `[@<list>] <item command>`: that list's open items, the insert being the item
  number (followed by ` ""` for `reword`) and the display the `t list` line with links shown as
  their text; `today`/`promote` offer only Later's items, `later`/`demote` only Today's.
  `use`, `lists rm`, `lists rename`: the lists as `<name> `, displayed as in `t lists`.
  `move <item>`: the lists as `@<name> `. `lists`: its subcommands. Anything else: nothing, with status 0. Only the item case reads (locks and
  tidies) a list; an unknown `@<list>` there fails.
- `t lists`: one line per list, `<* or space> <name padded> <n> open`, sorted by name.
  `t lists --json`: `[{"name":"Todo","open":3,"current":true}]`, one object per line.
- `t list`: one line per open item, `<n>  <section> <text>` (`printf "%3d  %-6s %s\n"`). On a
  terminal, with more than one list, a first line shows the list's name in bold.
  `[t:pick]` takes the item number from the first field of the line chosen in fzf.
- `t list --done`: `<YYYY-MM-DD>  <text>`, newest first.
- `t list --json`: a JSON array in `t list` order, one object per line:
  `{"list":"Todo","n":1,"section":"Today","text":"...","jira":["PROJ-1"]}`. `text` is the item as stored
  (Markdown). `jira` holds the configured Jira keys in it, once each, and is empty when linking is
  off. `[]` when there are no open items. Never rendered for a terminal.
- Success messages: `Added <n> (<section>): <text>`, `Done: <text>`, `Removed: <text>`, `Reworded <n> (<section>): <text>`,
  `Moved to <section> as <n>: <text>`, `Created <file>`,
  `Undid the last change; run t undo again to redo it`.

## Invariants

- **The list must exist.** Only `t init` (the default list) and `t lists add` (others) create
  lists: every command that works on a list dies when its file is missing, before locking.
- **Which list.** `@<list>` as the first argument, found by `[t:find_list]`, else
  `[t:current_name]`: `TODO_LIST` (an unknown name is an error), else the name in `$curfile`
  (`.current`; an unknown name there warns and falls back), else the default list. `@<list>` with
  `init`, `lists` or `use` is an error. The default list's file is `TODO_FILE` itself; any other
  list's is `$ldir/<name>.md`.
- **What a list is.** Every regular, non-hidden `*.md` file directly in `$ldir`, plus the default
  list, except names of the form `Archive <year>`. `[t:check_name]` refuses names that are empty,
  contain `/`, a tab or a newline, start with `.` or `@`, end in `.md` or look like `Archive <year>`;
  `t lists add` and `t lists rename` also refuse a name that matches an existing list ignoring case.
- **Current list.** `[t:use_cmd]` writes the name to `$curfile` through a temp file and `mv`, so
  readers never see half a name, and deletes it for the default list. It only ever runs under
  `[t:dir_lock]`.
- **Changing the lists.** `t lists add`, `rm`, `rename` and `t use` hold `[t:dir_lock]`; `rm` and
  `rename` also hold the list's own lock, taken after the folder's, so they never change a list
  another run is using. `rm` refuses the default list, the current list and, without `--force`, a
  list with open items; it deletes the file, `$bak`, `$sum` and the lock file, and keeps the
  archives. `rename` refuses the default list (TODO_FILE names it), moves the file, `$bak`, `$sum`
  and every `Archive/<old> <year>.md`, and updates `$curfile` when the list was current (and
  `TODO_LIST` isn't set).
- **`init` never overwrites.** `[t:init]` dies when anything, including a dangling symlink, is
  already at the target file. Otherwise it creates the directory (`mkdir -p`) and writes the three
  headings. The one exception is a vault: `t init --obsidian` (or accepting a vault in plain
  `t init`) uses an existing regular file there as it is.
- **Where `init` creates the list.** `t init <path>`: at `<path>` made absolute, then shows the
  `~/.zshenv` line for it when it isn't the list `t` uses already. `t init --obsidian`: at
  `<vault>/Todo/Todo.md`. Plain `t init`: at `$f`, but when stdin and stdout are a terminal,
  `TODO_FILE` is unset and Obsidian lists vaults, it first offers them.
- **`init` asks only in a terminal.** `[t:ask]` is only reached when stdin is a terminal, so a
  script or agent is never left waiting. Without one, `[t:pick_vault]` takes the only vault and
  dies listing them when there are several, and `[t:remember_list]` shows the line without writing.
- **Remembering the list.** `[t:remember_list]` writes `${ZDOTDIR:-$HOME}/.zshenv` only after a yes
  (the default) in a terminal. It appends a marker comment plus
  `export TODO_FILE="${TODO_FILE:-<path>}"` (`$HOME/...` when under the home directory, with
  `\ " $` and backticks escaped), replacing an earlier marked pair and the blank lines before it,
  and rewrites the file in place. It changes nothing when another line there mentions
  `TODO_FILE`, or when the marked line already names this list.
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
  list command, including `edit`, sees a tidied list. `help`, `version`, `commands`, `init`,
  `lists`, `use` and `complete` without items to offer exit before that; none of them reads a
  list's items or tidies one. `complete` for an item command reads, locks and tidies the list like
  `t list`.
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
- **Archive before rewriting.** `[t:tidy]` appends to every `$adir/<list> <year>.md` first
  (`$adir` is `Archive/` in the lists folder, created when needed) and calls `save` only if all of
  those writes worked; otherwise it warns and leaves the list untidied. A new archive file starts
  with a `# <list> archive <year>` heading; items are appended sorted, one file per list and year
  of the item's date. `t list --done --all` reads only the list's own archives.
- **Old archives.** Archives from before there were several lists, `$ldir/Archive <year>.md`,
  belong to the default list. Before any command that works on a list, `[t:migrate_archives]`
  moves each to `$adir/<default list> <year>.md`, or appends its items there when that file
  exists, under the default list's lock (in a subshell when another list was chosen). A failure
  warns and leaves the old file.
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
- **Moving between lists.** `t move` (`mv`) takes the target from its last argument, which must
  be `@<list>` naming another list; the arguments in between name the item in the source list.
  Before reading either list it takes both lists' locks in the order of their files' paths, so
  two moves in opposite directions can't deadlock, and tidies both. `[t:move_to]` inserts the item
  at the end of the same section in the target (dying, with nothing changed, when that heading is
  missing), saves the target, then removes it from the source and saves that: a failure in
  between leaves the item in both lists, never in neither. Each list's undo covers its own half.
- **Per list.** The lock, backup, checksum (so undo) and archives belong to one list, named after
  its file (`.<file>.lock`, `.<file>.bak`, `.<file>.sum`, `Archive/<list> <year>.md`).
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

- Rewrites the chosen list's file in place (`[t:save]`), from `add`, `done`, `today`, `later`,
  `rm`, `reword`, `pick` and from `[t:tidy]` on any list command when tidying changes the list.
  `t move` rewrites two lists, the target first.
- Overwrites `$bak` with the current list before every write, and `$sum` with the checksum of
  the new list after it (`[t:save]`); deletes `$bak` after a tidy that archived (`[t:tidy]`).
  `t undo` restores `$bak`.
- Creates `Archive/` in the lists folder and creates or appends to `Archive/<list> <year>.md`
  (`[t:tidy]`); reads them for `t list --done --all` (`[t:list_done]`). Moves old
  `Archive <year>.md` files there (`[t:migrate_archives]`).
- `t lists add` creates `<name>.md`; `t lists rm` deletes a list's file and hidden files;
  `t lists rename` moves them and its archives. `t use` writes or deletes `.current` in the lists
  folder. These hold `.lists.lock` there, an empty file that stays.
- Creates `$TODO_FILE` and its directory (`[t:init]`), only when nothing is there yet.
- Reads Obsidian's `obsidian.json` (`[t:obsidian_vaults]`) and appends to or rewrites
  `${ZDOTDIR:-$HOME}/.zshenv` (`[t:remember_list]`), both only for `t init`.
- Creates temp files with `mktemp` and removes them before returning.
- Runs external programs: `awk`, `date`, `cmp`, `cksum`, `cut`, `sort`, `grep`, `cat`, `mktemp`; `perl`
  when Jira linking is on (`[t:linkify]`) or stdout is a terminal (`[t:render]`); `fzf` for
  `t pick` and `t use` alone; `$EDITOR` (via `exec`) for `t edit`.
- Writes OSC 8 escape sequences to stdout when it is a terminal (`[t:render]`).
- Writes warnings and errors to stderr prefixed with `t: `; exits 1 on `[t:die]` and on an unknown
  command (after printing the usage to stderr). `t pick` exits with fzf's status when nothing is
  chosen.
- Creates `$lockfile`, an empty file next to the list, and holds an fcntl lock on it while running.
  The file stays; only the lock is released.
- No network access.

## Verification

- Run: `test/run.zsh` (from the repository root)
