# Tab completion for t. Source this from ~/.zshrc after fzf's key bindings (source <(fzf --zsh)).
#
# With the cursor at the end of "t " or "t <partial command>", Tab opens fzf with every t command,
# a description and an example (from `t commands`). The chosen command replaces the line; for
# `add` the cursor lands between the quotes. Anywhere else Tab does whatever it did before.

# Whatever Tab was bound to before (fzf-completion when fzf's bindings are loaded).
typeset -g _t_tab_fallback=${${(z)"$(bindkey '^I')"}[2]:-expand-or-complete}

_t_tab() {
  if [[ -z $RBUFFER && $LBUFFER =~ '^([[:space:]]*)t +([^[:space:]]*)$' ]]; then
    local indent=$match[1] query=$match[2] pick ins
    pick=$(command t commands | fzf --height=40% --reverse --delimiter=$'\t' --with-nth=2 \
      --query="$query" --prompt='t ' --header='Pick a command (Enter inserts it, Esc cancels)')
    if [[ -n $pick ]]; then
      ins=${pick%%$'\t'*}
      if [[ $ins == *'""' ]]; then
        LBUFFER="${indent}t ${ins%\"}"; RBUFFER='"'
      else
        LBUFFER="${indent}t $ins"
      fi
    fi
    zle reset-prompt
  else
    zle "$_t_tab_fallback"
  fi
}
zle -N _t_tab
bindkey '^I' _t_tab
