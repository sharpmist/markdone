# shell

## Overview

`t.zsh` adds Tab completion for `t` to an interactive zsh. It is sourced from `~/.zshrc`. With the
cursor at the end of `t ` or `t <partial command>`, Tab opens fzf with every `t` command and puts
the chosen one on the command line. After a command that takes an item (`t done `, `t today fix`),
Tab opens fzf with the open items that command can take and inserts the chosen item's number.
Anywhere else, Tab does what it did before.

## Interface

- `[t.zsh:_t_tab]`: zle widget, registered with `zle -N _t_tab` and bound to Tab
  (`bindkey '^I' _t_tab`).
- `[t.zsh:_t_insert]`: puts an fzf choice's insert text on the command line after a given start.
- `[t.zsh:_t_tab_fallback]`: global holding the widget Tab was bound to when the file was sourced
  (default `expand-or-complete`).

## Invariants

- **Source order.** `_t_tab_fallback` is read from `bindkey '^I'` at source time, so the file must
  be sourced after fzf's own key bindings (`source <(fzf --zsh)`) for those to stay reachable.
- **Command trigger.** The command menu opens only when `$RBUFFER` is empty and `$LBUFFER` matches
  `^([[:space:]]*)t +([^[:space:]]*)$`.
- **Item trigger.** Otherwise, the item menu opens only when `$RBUFFER` is empty, `$LBUFFER`
  matches `^([[:space:]]*)t +([^[:space:]]+) +([^[:space:]"']*)$` (a command and at most a partial
  first argument, without quotes), and `command t complete <command>` prints at least one line.
  Every other Tab press, including one where `t complete` fails or prints nothing, calls
  `zle "$_t_tab_fallback"`.
- **Input format.** The menus are filled from `command t commands` (one line per command) and
  `command t complete <command>` (one line per item), both `<insert text>\t<display text>`. fzf
  shows the second field (`--with-nth=2`); the first field is inserted after `t ` or
  `t <command> `. See [`../bin/EGM.md`](../bin/EGM.md).
- **Quoted inserts.** An insert text ending in `""` is split so the cursor lands between the
  quotes (`LBUFFER` gets the opening quote, `RBUFFER` the closing one).
- **Pre-filled query.** Text typed after `t ` (or after `t <command> `) is passed to fzf as
  `--query`.
- **Line replacement.** The choice replaces the whole line as `<indent>t <insert text>` or
  `<indent>t <command> <insert text>`, keeping any leading whitespace and collapsing other runs of
  spaces. Cancelling fzf leaves the line as it was.
- **No command list here.** Commands are never hardcoded in this file. Adding a command only needs
  a change to `commands()` in `bin/t`, plus `ITEM_CMDS` when it takes an item.
- `t` must be on `PATH` (`command t`), and `fzf` must be installed.

## Side Effects

- Rebinds Tab (`^I`) in the current zsh session and defines the global `_t_tab_fallback`.
- Runs `t commands` and `fzf` on a command-menu Tab press; `t commands` exits before the list is
  read. On an item-trigger Tab press it runs `t complete <command>`, with stderr discarded. For a
  command without items that exits before the list is read; for an item command it locks and
  tidies the list like `t list`, and then runs `fzf`.
- Replaces `LBUFFER` and `RBUFFER` when a command or item is picked. Calls `zle reset-prompt`
  after every fzf menu, picked or cancelled.

## Verification

- Run: `test/run.zsh` (from the repository root; the Tab completion checks need zsh's `zsh/zpty`
  and `zsh/zselect` modules)
