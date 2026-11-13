# Tab completion for t. Source this from ~/.zshrc after fzf's key bindings (source <(fzf --zsh)).
#
# With the cursor at the end of "t " or "t <partial command>", Tab opens fzf with every t command,
# a description and an example (from `t commands`). The chosen command replaces the line; for
# `add` the cursor lands between the quotes. After a command that takes an item ("t done ",
# "t today fix"), Tab opens fzf with the open items it can take (from `t complete <command>`) and
# inserts the chosen item's number. Anywhere else Tab does whatever it did before.

# Whatever Tab was bound to before (fzf-completion when fzf's bindings are loaded).
typeset -g _t_tab_fallback=${${(z)"$(bindkey '^I')"}[2]:-expand-or-complete}

# _t_insert <line start> <fzf choice>: put the choice's insert text (before the tab) after the line
# start; an insert ending in "" leaves the cursor between the quotes.
_t_insert() {
  local ins=${2%%$'\t'*}
  if [[ $ins == *'""' ]]; then
    LBUFFER="$1${ins%\"}"; RBUFFER='"'
  else
    LBUFFER="$1$ins"
  fi
}

_t_tab() {
  local pick items
  if [[ -z $RBUFFER && $LBUFFER =~ '^([[:space:]]*)t +([^[:space:]]*)$' ]]; then
    local indent=$match[1] query=$match[2]
    pick=$(command t commands | fzf --height=40% --reverse --delimiter=$'\t' --with-nth=2 \
      --query="$query" --prompt='t ' --header='Pick a command (Enter inserts it, Esc cancels)')
    [[ -n $pick ]] && _t_insert "${indent}t " "$pick"
    zle reset-prompt
  elif [[ -z $RBUFFER && $LBUFFER =~ '^([[:space:]]*)t +([^[:space:]]+) +([^[:space:]"'\'']*)$' ]] &&
       items=$(command t complete "$match[2]" 2>/dev/null) && [[ -n $items ]]; then
    local indent=$match[1] cmd=$match[2] query=$match[3]
    pick=$(print -r -- "$items" | fzf --height=40% --reverse --delimiter=$'\t' --with-nth=2 \
      --query="$query" --prompt="t $cmd " --header='Pick an item (Enter inserts its number, Esc cancels)')
    [[ -n $pick ]] && _t_insert "${indent}t $cmd " "$pick"
    zle reset-prompt
  else
    zle "$_t_tab_fallback"
  fi
}
zle -N _t_tab
bindkey '^I' _t_tab
