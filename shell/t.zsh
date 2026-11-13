# Tab completion for t. Source this from ~/.zshrc after fzf's key bindings (source <(fzf --zsh)).
#
# With the cursor at the end of "t " or "t <partial command>", Tab opens fzf with every t command,
# a description and an example (from `t commands`). The chosen command replaces the line; for
# `add` the cursor lands between the quotes. "t @<partial>" offers the lists instead. After more
# words ("t done ", "t @work today fix", "t use "), Tab asks `t complete <words>` what fits there
# (a list's open items, the lists, ...) and offers that. Anywhere else Tab does whatever it did
# before.

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

# _t_menu <prompt> <header> <query>: show stdin's "<insert>\t<display>" lines in fzf; prints the choice.
_t_menu() {
  fzf --height=40% --reverse --delimiter=$'\t' --with-nth=2 --query="$3" --prompt="$1" --header="$2"
}

_t_tab() {
  local pick choices
  if [[ -z $RBUFFER && $LBUFFER =~ '^([[:space:]]*)t +(@?)([^[:space:]]*)$' ]]; then
    local indent=$match[1] at=$match[2] query=$match[3]
    if [[ -n $at ]]; then
      choices=$(command t complete @ 2>/dev/null)
      [[ -n $choices ]] && pick=$(print -r -- "$choices" |
        _t_menu 't @' 'Pick a list (Enter inserts it, Esc cancels)' "$query")
    else
      pick=$(command t commands | _t_menu 't ' 'Pick a command (Enter inserts it, Esc cancels)' "$query")
    fi
    [[ -n $pick ]] && _t_insert "${indent}t " "$pick"
    zle reset-prompt
  elif [[ -z $RBUFFER && $LBUFFER =~ '^([[:space:]]*)t +([^[:space:]].*[^[:space:]]|[^[:space:]]) +([^[:space:]"'\'']*)$' ]] &&
       choices=$(command t complete "${(@Q)${(z)match[2]}}" 2>/dev/null) && [[ -n $choices ]]; then
    local indent=$match[1] words=$match[2] query=$match[3]
    pick=$(print -r -- "$choices" | _t_menu "t $words " 'Pick one (Enter inserts it, Esc cancels)' "$query")
    [[ -n $pick ]] && _t_insert "${indent}t $words " "$pick"
    zle reset-prompt
  else
    zle "$_t_tab_fallback"
  fi
}
zle -N _t_tab
bindkey '^I' _t_tab
