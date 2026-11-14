# shell

## Overview

`t.zsh` adds Tab completion for `t` to an interactive zsh. It is sourced from `~/.zshrc`. With the
cursor at the end of `t ` or `t <partial command>`, Tab opens fzf with every `t` command and puts
the chosen one on the command line; after `t @` it offers the lists. After more words (`t done `,
`t @work today fix`, `t use `), Tab asks `t complete <words>` what fits there (a list's open items,
the lists, a subcommand) and offers that. Anywhere else, Tab does what it did before.

## Interface

- `[t.zsh:_t_tab]`: zle widget, registered with `zle -N _t_tab` and bound to Tab
  (`bindkey '^I' _t_tab`).
- `[t.zsh:_t_insert]`: puts an fzf choice's insert text on the command line after a given start.
- `[t.zsh:_t_menu]`: runs fzf on `<insert>\t<display>` lines, showing the display field.
- `[t.zsh:_t_tab_fallback]`: global holding the widget Tab was bound to when the file was sourced
  (default `expand-or-complete`).

## Invariants

- **Source order.** `_t_tab_fallback` is read from `bindkey '^I'` at source time, so the file must
  be sourced after fzf's own key bindings (`source <(fzf --zsh)`) for those to stay reachable.
- **First-word trigger.** When `$RBUFFER` is empty and `$LBUFFER` matches
  `^([[:space:]]*)t +(@?)([^[:space:]]*)$`, the menu shows `command t commands`, or
  `command t complete @` (the lists) when the word starts with `@`.
- **Later-word trigger.** Otherwise, when `$RBUFFER` is empty and `$LBUFFER` is `t`, one or more
  words and at most a partial word without quotes
  (`^([[:space:]]*)t +([^[:space:]].*[^[:space:]]|[^[:space:]]) +([^[:space:]"']*)$`), the words
  are split as the shell would (`${(@Q)${(z)…}}`) and passed to `command t complete`. `t`
  decides what fits; the widget hardcodes nothing about it.
- **Fallback.** Every other Tab press, including one where `t complete` fails or prints nothing,
  calls `zle "$_t_tab_fallback"`.
- **Input format.** Every menu's lines are `<insert text>\t<display text>`. fzf shows the second
  field (`--with-nth=2`); the first field is inserted after `t ` or `t <words> `. See
  [`../src/EGM.md`](../src/EGM.md).
- **Quoted inserts.** An insert text ending in `""` is split so the cursor lands between the
  quotes (`LBUFFER` gets the opening quote, `RBUFFER` the closing one).
- **Pre-filled query.** The partial word (after `t `, `t @` or `t <words> `) is passed to fzf as
  `--query`.
- **Line replacement.** The choice replaces the whole line as `<indent>t <insert text>` or
  `<indent>t <words> <insert text>`, keeping any leading whitespace and collapsing the spaces
  before the partial word. Cancelling fzf leaves the line as it was.
- **No command list here.** Commands are never hardcoded in this file. Adding a command only needs
  a change to `COMMANDS` in `src/complete.rs`, plus `ITEM_CMDS` or `words()` there when Tab should
  offer something after it.
- `t` must be on `PATH` (`command t`), and `fzf` must be installed.

## Side Effects

- Rebinds Tab (`^I`) in the current zsh session and defines the global `_t_tab_fallback`.
- Runs `t commands` or `t complete …` and `fzf` on a matching Tab press, with `t complete`'s
  stderr discarded. Only when it offers a list's items does `t complete` read a list, locking and
  tidying it like `t list`; otherwise no list is read.
- Replaces `LBUFFER` and `RBUFFER` when a command or item is picked. Calls `zle reset-prompt`
  after every fzf menu, picked or cancelled.

## Verification

- Run: `cargo build`, then `test/run.zsh` (from the repository root; the Tab completion checks need
  zsh's `zsh/zpty` and `zsh/zselect` modules). `cargo test` checks `t commands` and `t complete`.
