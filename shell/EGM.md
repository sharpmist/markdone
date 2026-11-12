# shell

## Overview

`t.zsh` adds Tab completion for `t` to an interactive zsh. It is sourced from `~/.zshrc`. With the
cursor at the end of `t ` or `t <partial command>`, Tab opens fzf with every `t` command and puts
the chosen one on the command line. Anywhere else, Tab does what it did before.

## Interface

- `[t.zsh:_t_tab]`: zle widget, registered with `zle -N _t_tab` and bound to Tab
  (`bindkey '^I' _t_tab`).
- `[t.zsh:_t_tab_fallback]`: global holding the widget Tab was bound to when the file was sourced
  (default `expand-or-complete`).

## Invariants

- **Source order.** `_t_tab_fallback` is read from `bindkey '^I'` at source time, so the file must
  be sourced after fzf's own key bindings (`source <(fzf --zsh)`) for those to stay reachable.
- **Trigger.** The fzf menu opens only when `$RBUFFER` is empty and `$LBUFFER` matches
  `^([[:space:]]*)t +([^[:space:]]*)$`. Every other Tab press calls `zle "$_t_tab_fallback"`.
- **Input format.** The menu is filled from `command t commands`, one line per command:
  `<insert text>\t<display text>`. fzf shows the second field (`--with-nth=2`); the first field is
  inserted after `t `. See [`../bin/EGM.md`](../bin/EGM.md).
- **Quoted inserts.** An insert text ending in `""` is split so the cursor lands between the
  quotes (`LBUFFER` gets the opening quote, `RBUFFER` the closing one).
- **Pre-filled query.** Text typed after `t ` is passed to fzf as `--query`.
- **Line replacement.** The chosen command replaces the whole line as `<indent>t <insert text>`,
  keeping any leading whitespace. Cancelling fzf leaves the line as it was.
- **No command list here.** Commands are never hardcoded in this file; adding a command only
  needs a change to `commands()` in `bin/t`.
- `t` must be on `PATH` (`command t`), and `fzf` must be installed.

## Side Effects

- Rebinds Tab (`^I`) in the current zsh session and defines the global `_t_tab_fallback`.
- Runs `t commands` and `fzf` on a matching Tab press. `t commands` exits before the list is
  read, so the todo list is neither read nor changed.
- Replaces `LBUFFER` and `RBUFFER` when a command is picked. Calls `zle reset-prompt` after every
  fzf menu, picked or cancelled.

## Verification

- Run: `test/run.zsh` (from the repository root; the Tab completion checks need zsh's `zsh/zpty`
  and `zsh/zselect` modules)
