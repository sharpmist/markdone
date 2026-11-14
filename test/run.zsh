#!/usr/bin/env zsh
# Runs t against throwaway files and checks output, exit codes and file contents.
# Usage: test/run.zsh   (exits non-zero if any check fails)
# The checks that need zsh or a terminal; everything else is in tests/*.rs (cargo test).
#   T_BIN=/abs/path/to/t test/run.zsh   tests another build of t (default: target/debug/t)

repo=${0:A:h:h}
T_BIN=${T_BIN:-$repo/target/debug/t}
[[ $T_BIN == /* ]] || { print -r -- "T_BIN must be an absolute path (got '$T_BIN')" >&2; exit 2; }
[[ -x $T_BIN ]] || { print -r -- "t isn't built at $T_BIN; run cargo build first" >&2; exit 2; }
zmodload zsh/datetime
days_ago() { local noon; strftime -r -s noon '%F %T' "$(strftime %F $EPOCHSECONDS) 12:00:00"; strftime %F $(( noon - $1 * 86400 )); }
today=$(date +%F); d7=$(days_ago 7); d8=$(days_ago 8)
export TODO_JIRA_URL="https://jira.example.com/browse/" TODO_JIRA_KEYS="PROJ OPS WEB"
pass=0; fail=0

check() { # <name> <expected> <actual>
  if [[ "$2" == "$3" ]]; then (( pass++ ))
  else (( fail++ )); print -r -- "FAIL: $1"; print -r -- "  expected: ${(q+)2}"; print -r -- "  actual:   ${(q+)3}"; fi
}
run() { out=$("$T_BIN" "$@" 2>&1); rc=$?; }  # sets $out and $rc
# Temp dirs are removed when the suite exits, by this process only: zpty sessions are forks of it
# that inherit the trap, and when they end they must not delete the dirs the suite still uses.
zmodload zsh/system
suite_pid=$sysparams[pid]
dirs=(); trap '(( sysparams[pid] == suite_pid )) && { chmod -R u+w $dirs 2>/dev/null; rm -rf $dirs; }' EXIT
# t looks into $ZDOTDIR/.zshenv (default ~/.zshenv) for the upgrade warning, and zsh reads it in the
# shells this script starts. Point it at an empty dir so the user's settings can't get in.
export ZDOTDIR=$(mktemp -d); dirs+=$ZDOTDIR
# A throwaway home as well, for everything this script and t run. Without it, a test that forgets to
# set HOME reaches the real one: t init reads Obsidian's list of vaults there and can create a list
# in a real vault; ./install links into ~/.local/bin and edits ~/.zshrc.
real_home=$HOME
export HOME=$(mktemp -d); dirs+=$HOME
# t reads a config file from $XDG_CONFIG_HOME (or %APPDATA% on Windows) and t init can write
# it. Point both into the throwaway home too, so the real config is never read or changed.
export XDG_CONFIG_HOME=$HOME/.config APPDATA=$HOME/AppData
fresh() { dir=$(mktemp -d); dirs+=$dir; export TODO_FILE="$dir/Todo.md"; print -rn -- "$1" >"$TODO_FILE"; }
file() { cat "$TODO_FILE"; }
EMPTY=$'## Today\n\n## Later\n\n## Done\n'

# --- the tests' own safety ---
[[ $HOME != "$real_home" && -d $HOME && -z "$(ls -A "$HOME")" ]]
check "tests run with an empty, throwaway HOME" 0 $?
[[ $XDG_CONFIG_HOME == $HOME/* && $APPDATA == $HOME/* ]]
check "the config file t may read or write is inside the throwaway HOME" 0 $?
out=$(unset TODO_FILE; cd /; "$T_BIN" init --obsidian </dev/null 2>&1)
check "t init --obsidian, run without its own HOME, sees no real vaults" \
  "t: found no Obsidian vaults (Obsidian lists a vault once you've opened it); use t init <path> instead" "$out"

# --- output on a terminal (needs a pseudo-terminal, so it stays here) ---
on_tty() { python3 -I -c 'import pty,sys; pty.spawn(sys.argv[1:])' "$T_BIN" "$@" </dev/null | tr -d '\r'; }  # run t with a pseudo-terminal as stdout
osc() { print -rn -- $'\e]8;;'"https://jira.example.com/browse/$1"$'\e\\'"$1"$'\e]8;;\e\\'; }
fresh "## Today
- [ ] Typed in an editor PROJ-10

## Later

## Done
- [x] $today Shipped WEB-7
"
out=$(on_tty list)
check "list on a terminal shows clickable keys" "  1  Today  Typed in an editor $(osc PROJ-10)" "$out"
out=$(on_tty list --done)
check "list --done on a terminal shows clickable keys" "$today  Shipped $(osc WEB-7)" "$out"
out=$(on_tty list --json)
[[ $out == "["* && $out != *$'\e'* ]]; check "json: plain even on a terminal" 0 $?
run lists add Work
out=$(on_tty list); check "list on a terminal names the list when there are several" $'\e[1mTodo\e[0m' "${out%%$'\n'*}"

# --- Tab completion widget (shell/t.zsh), driven through a pseudo-terminal ---
zmodload zsh/zpty zsh/zselect
fresh "## Today
- [ ] Fix login
- [ ] Fix logout

## Later
- [ ] Clean up

## Done
"
printf '## Today\n- [ ] Ship it\n\n## Later\n\n## Done\n' >"$dir/Work.md"
widget_dump=$(mktemp); dirs+=$widget_dump
zpty -b TT "PATH='${T_BIN:h}':\$PATH PS1='READY> ' zsh -f -i"
ready() { # wait (max ~5s, never blocks) until the shell shows its prompt again
  local buf= chunk i
  for i in {1..50}; do
    while zpty -r -t TT chunk 2>/dev/null; do buf+=$chunk; done
    [[ $buf == *'READY> '* ]] && return 0
    zselect -t 10
  done
  print -r -- "harness: no prompt; got ${(q+)buf}"; return 1
}
setup() { zpty -w TT "$1"; ready; }
ready
setup "bindkey -e; _mark() { LBUFFER+=X }; zle -N _mark; bindkey '^I' _mark"
setup "fzf() { local a q; for a; do [[ \$a == --query=* ]] && q=\${a#--query=}; done; grep -m1 \"^\$q\" }"
setup "_dump() { print -r -- \"\$BUFFER|\$CURSOR\" >'$widget_dump'; BUFFER=; }; zle -N _dump; bindkey '^X^D' _dump"
setup "source '$repo/shell/t.zsh'"
tab() { # <typed text>: type it, press Tab, dump the line; sets $got to "BUFFER|CURSOR"
  : >"$widget_dump"; zpty -w -n TT "$1"$'\t\C-x\C-d'
  local i chunk; for i in {1..50}; do
    while zpty -r -t TT chunk 2>/dev/null; do :; done   # keep draining the terminal or zle stalls
    [[ -s $widget_dump ]] && break; zselect -t 10
  done
  got=$(<"$widget_dump")
}
tab 't ';        check "Tab after 't ' picks a command"              't list|6'      "$got"
tab 't ad';      check "Tab after 't ad': cursor inside the quotes"   't add ""|7'    "$got"
tab 't do';      check "Tab after 't do' inserts 'done '"             't done |7'     "$got"
tab 't to';      check "Tab after 't to' inserts 'today '"            't today |8'    "$got"
tab 't la';      check "Tab after 't la' inserts 'later '"            't later |8'    "$got"
tab 'echo hi';   check "Tab elsewhere falls back to the old binding"  'echo hiX|8'    "$got"
tab 't add foo'; check "Tab after a full t command falls back"        't add fooX|10' "$got"
tab 't';         check "Tab on bare 't' falls back"                   'tX|2'          "$got"
tab 't done ';   check "Tab after 't done ' inserts an item's number" 't done 1|8'    "$got"
tab 't d 2';     check "Tab after an alias and a partial item"         't d 2|5'       "$got"
tab 't today ';  check "Tab after 't today ' offers Later's items"     't today 3|9'   "$got"
tab 't later ';  check "Tab after 't later ' offers Today's items"     't later 1|9'   "$got"
tab 't reword '; check "Tab after 't reword ': cursor in the quotes"   't reword 1 ""|12' "$got"
tab 't  rm  ';   check "Tab after an item command tidies the spacing"  't rm 1|6'      "$got"
tab 't done 1 '; check "Tab after the item falls back"                 't done 1 X|10' "$got"
tab 't list ';   check "Tab after a command without items falls back" 't list X|8'    "$got"
tab 't @';       check "Tab after 't @' picks a list"                 't @Todo |8'    "$got"
tab 't @Work ';  check "Tab after 't @Work ' picks a command"          't @Work list|12' "$got"
tab 't @Work done '; check "Tab after 't @Work done ' offers Work's items" 't @Work done 1|14' "$got"
tab 't use ';    check "Tab after 't use ' picks a list"               't use Todo |11' "$got"
tab 't lists ';  check "Tab after 't lists ' picks a subcommand"       't lists add ""|13' "$got"
tab 't @Nope done '; check "Tab after an unknown list falls back"      't @Nope done X|14' "$got"
setup "export TODO_FILE='$dir/none.md'"
tab 't done ';   check "Tab with no list falls back"                   't done X|8'    "$got"
zpty -d TT

# --- edit ---
fresh "$EMPTY"
out=$(EDITOR="echo opened" "$T_BIN" edit 2>&1)
check "EDITOR with arguments" "opened $TODO_FILE" "$out"

# --- the lock while t edit and t pick hand over to another program ---
# $lockbin/trylock <lock file> prints "free" when the lock can be taken, else "held". It takes a
# flock lock, as t does (zsh's own zsystem flock is fcntl, which Linux keeps apart from flock).
# (The other locking checks are in tests/locking.rs.)
lockbin=$(mktemp -d); dirs+=$lockbin
print -r -- '#!/bin/sh
exec python3 -I -c '\''
import fcntl, sys
f = open(sys.argv[1], "a")
try:
    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB); print("free")
except OSError:
    print("held")
'\'' "$1"' >"$lockbin/trylock"
chmod +x "$lockbin/trylock"
lock_of() { print -r -- "${TODO_FILE:h}/.${TODO_FILE:t}.lock"; }

# t edit execs the editor, which closes the lock's descriptor. The editor checks from a child
# process: the editor process itself (t after exec) could always relock a lock it still held.
fresh "$EMPTY"
cat >"$dir/editor" <<EOS
#!/bin/sh
"$lockbin/trylock" "\$(dirname "\$1")/.\$(basename "\$1").lock"
EOS
chmod +x "$dir/editor"
out=$(EDITOR="$dir/editor" "$T_BIN" edit 2>&1)
check "the editor started by t edit doesn't hold the lock" free "$out"


# --- pick ---
# A fake fzf: runs $PICK_DURING (as if another run happened while fzf was open), then prints the
# first input line containing $PICK, or exits 130 like a cancelled fzf when $PICK is empty.
fakebin=$(mktemp -d); dirs+=$fakebin
cat >"$fakebin/fzf" <<'EOS'
#!/bin/sh
input=$(cat)
[ -n "$PICK_DURING" ] && eval "$PICK_DURING" >/dev/null 2>&1
[ -n "$PICK" ] || exit 130
printf '%s\n' "$input" | grep -m1 -F -- "$PICK"
EOS
chmod +x "$fakebin/fzf"
run_pick() { # <text of the line to pick> [command to run while fzf is open]
  out=$(PATH="$fakebin:$PATH" T_BIN="$T_BIN" T_LOCK="$(lock_of)" T_TRYLOCK="$lockbin/trylock" PICK="$1" PICK_DURING="$2" \
    "$T_BIN" pick 2>&1); rc=$?
}
ABC=$'## Today\n- [ ] A\n- [ ] B\n\n## Later\n- [ ] C\n\n## Done\n'
fresh "$ABC"
run_pick B; check "pick marks the chosen item done" "0|Done: B" "$rc|$out"

fresh "$ABC"
run_pick C '"$T_TRYLOCK" "$T_LOCK" >"$T_LOCK.state"; "$T_BIN" rm A'
check "pick: the lock is free while fzf is open" free "$(<"$(lock_of).state")"
check "pick: numbers shifted while fzf was open" "0|Done: C" "$rc|$out"
check "pick: the other run's change is kept" $'## Today\n- [ ] B\n\n## Later\n\n## Done\n'"- [x] $today C" "$(file)"

fresh "$ABC"
run_pick C '"$T_BIN" rm C'
check "pick: item removed while fzf was open" \
  "1|t: 'C' is no longer an open item; it was changed while fzf was open, so nothing is done" "$rc|$out"
check "pick: nothing done when the item is gone" $'## Today\n- [ ] A\n- [ ] B\n\n## Later\n\n## Done' "$(file)"

fresh $'## Today\n- [ ] same\n\n## Later\n- [ ] same\n\n## Done\n'
run_pick "Later  same"
check "pick: of two items with the same text, the chosen one" \
  $'0|Done: same|## Today\n- [ ] same\n\n## Later\n\n## Done\n'"- [x] $today same" "$rc|$out|$(file)"

fresh $'## Today\n- [ ] same\n- [ ] x\n\n## Later\n- [ ] same\n\n## Done\n'
before=$(file)
run_pick "Later  same" '"$T_BIN" rm x'
check "pick: no longer clear which item was meant" "1|t: 'same' now matches 2 open items; run t pick again" "$rc|$out"

fresh "$ABC"
run_pick ""; check "pick: cancelling fzf changes nothing" "130|$ABC" "$rc|$(file)"$'\n'
fresh "$EMPTY"
run_pick B; check "pick with no open items" "1|t: no open items to pick" "$rc|$out"

nofzf=$(mktemp -d); dirs+=$nofzf
for c in zsh awk perl date cat mktemp cmp cut sort grep touch rm; do ln -s "${commands[$c]}" "$nofzf/$c"; done
fresh "$ABC"
out=$(PATH="$nofzf" "$T_BIN" pick 2>&1); rc=$?
check "pick without fzf" "1|t: t pick needs fzf (https://github.com/junegunn/fzf)" "$rc|$out"

# --- the README demo stays true ---
# Replays every "$ t ..." line of the README's console block and compares the whole transcript.
demo=$(awk '/^```console$/{on=1; next} on && /^```$/{exit} on' "$repo/README.md")
fresh "$EMPTY"
replay=$(for cmd in ${(f)"$(grep '^\$ t ' <<<"$demo")"}; do
  print -r -- "$cmd"; TODO_JIRA_KEYS= eval "\"\$T_BIN\" ${cmd#\$ t }" 2>&1
done)
check "README demo matches real output" "$demo" "$replay"

# --- ./install ---
# Always with a throwaway home (./install edits ~/.zshrc and links into ~/.local/bin), and from a
# throwaway copy of the repo with a fake cargo, so no real build runs: the fake one logs its
# arguments to $copy/cargo.log and makes target/release/t (or fails when CARGO_FAIL is set).
copy=$(mktemp -d); dirs+=$copy; copy=${copy:A}
mkdir -p "$copy/shell"; cp "$repo/install" "$copy/"; cp "$repo/shell/t.zsh" "$copy/shell/"
built=$copy/target/release/t
cat >"$fakebin/cargo" <<'EOS'
#!/bin/sh
echo "$@" >>cargo.log
[ -n "$CARGO_FAIL" ] && exit 1
mkdir -p target/release && printf '#!/bin/sh\n' >target/release/t && chmod +x target/release/t
EOS
chmod +x "$fakebin/cargo"
inst() { out=$(cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData PATH="$home/.local/bin:$fakebin:$PATH" "$copy/install" "$@" 2>&1); rc=$?; }
DONE="Done. Open a new terminal, then run 't init' to create your list (or set TODO_FILE first). After a git pull, run ./install again to rebuild t."
home=$(mktemp -d); dirs+=$home
print -r -- 'source <(fzf --zsh)' >"$home/.zshrc"
inst
check "install: builds, links t and turns on Tab completion" "0|Linked $home/.local/bin/t -> $built
Turned on Tab completion: added to the end of $home/.zshrc:
  source '$copy/shell/t.zsh'
$DONE" "$rc|$out"
check "install: builds with cargo build --release --locked" "build --release --locked" "$(<"$copy/cargo.log")"
[[ ${${:-$home/.local/bin/t}:A} == $built ]]; check "install: the link points at this clone's binary" 0 $?
check "install: ~/.zshrc gets one marked source line" "source <(fzf --zsh)

# markdone: Tab completion for t (remove with ./install --uninstall)
source '$copy/shell/t.zsh'" "$(<"$home/.zshrc")"
before=$(<"$home/.zshrc"); inst
check "install again: rebuilds, nothing else to do" "0|t is already linked: $home/.local/bin/t
Tab completion is already on in $home/.zshrc
$DONE" "$rc|$out"
check "install again: ~/.zshrc unchanged" "$before" "$(<"$home/.zshrc")"
inst --uninstall
check "uninstall removes the link and the line" "0|Removed $home/.local/bin/t
Removed Tab completion from $home/.zshrc" "$rc|$out"
check "uninstall leaves the rest of ~/.zshrc as it was" "source <(fzf --zsh)" "$(<"$home/.zshrc")"
inst --uninstall; check "uninstall again: nothing to do" "0|No link to this t at $home/.local/bin/t; left alone" "$rc|$out"

# Upgrading from 0.3, whose link pointed at the zsh script bin/t (gone now).
home=$(mktemp -d); dirs+=$home; mkdir -p "$home/.local/bin"; ln -s "$copy/bin/t" "$home/.local/bin/t"
inst --no-zshrc
check "install over 0.3: the old link is replaced" "0|Linked $home/.local/bin/t -> $built" "$rc|${${(@f)out}[1]}"
ln -sf "$copy/bin/t" "$home/.local/bin/t"; inst --uninstall
check "uninstall removes 0.3's link too" "Removed $home/.local/bin/t" "${out%%$'\n'*}"

home=$(mktemp -d); dirs+=$home
print -r -- $'alias t="tmux"\nsource <(fzf --zsh)' >"$home/.zshrc"
inst --no-zshrc
check "install warns about another t in ~/.zshrc" "0|install: $home/.zshrc defines its own t, which hides this one; rename or remove it:
1:alias t=\"tmux\"" "$rc|${(F)${(@f)out}[2,3]}"
check "--no-zshrc leaves ~/.zshrc alone" $'alias t="tmux"\nsource <(fzf --zsh)' "$(<"$home/.zshrc")"

home=$(mktemp -d); dirs+=$home; mkdir -p "$home/.local/bin"; print x >"$home/.local/bin/t"
inst; check "install refuses to replace another t" \
  "1|install: $home/.local/bin/t already exists and isn't this t; remove it, or choose another directory with --bin" "$rc|$out"
inst --uninstall; check "uninstall leaves another t alone" "x" "$(<"$home/.local/bin/t")"

home=$(mktemp -d); dirs+=$home
inst --bin "$home/elsewhere"
check "install --bin off PATH: links there, warns" "0|Linked $home/elsewhere/t -> $built|install: $home/elsewhere isn't on your PATH; add it, e.g. in ~/.zshenv: export PATH=\"$home/elsewhere:\$PATH\"" \
  "$rc|${(j:|:)${(@f)out}[1,2]}"
[[ $out == *"doesn't seem to load fzf's key bindings"* ]]; check "install warns when ~/.zshrc doesn't load fzf" 0 $?

# Building: without cargo, a failed build, and --no-build.
home=$(mktemp -d); dirs+=$home
nocargo=$(mktemp -d); dirs+=$nocargo
for c in zsh mkdir ln sed grep awk cat rm mktemp; do ln -sf "${commands[$c]}" "$nocargo/$c"; done
out=$(cd /; HOME=$home ZDOTDIR=$home PATH="$nocargo" "$copy/install" 2>&1); rc=$?
check "install without cargo fails" "1|install: cargo isn't installed; get Rust from https://rustup.rs and run ./install again" "$rc|$out"
out=$(cd /; HOME=$home ZDOTDIR=$home PATH="$nocargo" "$copy/install" --no-build --no-zshrc 2>&1); rc=$?
check "install --no-build links what is built" "0|Linked $home/.local/bin/t -> $built" "$rc|${${(@f)out}[1]}"
home=$(mktemp -d); dirs+=$home
out=$(cd /; HOME=$home ZDOTDIR=$home PATH="$fakebin:$PATH" CARGO_FAIL=1 "$copy/install" 2>&1); rc=$?
check "install: a failed build stops it" "1|install: cargo build failed; see the messages above" "$rc|$out"
[[ ! -e $home/.local/bin/t ]]; check "install: a failed build links nothing" 0 $?
rm -rf "$copy/target"
out=$(cd /; HOME=$home ZDOTDIR=$home PATH="$nocargo" "$copy/install" --no-build 2>&1); rc=$?
check "install --no-build without a build fails" "1|install: $built isn't built; run ./install without --no-build" "$rc|$out"

home=$(mktemp -d); dirs+=$home
for c in mkdir ln sed chmod; do ln -sf "${commands[$c]}" "$nofzf/$c"; done
ln -sf "$fakebin/cargo" "$nofzf/cargo"
out=$(cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData PATH="$home/.local/bin:$nofzf" "$copy/install" 2>&1); rc=$?
check "install without fzf: Tab completion left off" "0|install: fzf isn't installed, so Tab completion is left off; install fzf and run ./install again" "$rc|${${(f)out}[2]}"
[[ ! -e $home/.zshrc ]]; check "install without fzf: no ~/.zshrc written" 0 $?

inst --help; [[ $rc == 0 && $out == *"./install --uninstall"* && $out == *"--no-build"* ]]; check "install --help" 0 $?
inst --bogus; check "install: unknown option fails" "1|install: unknown option '--bogus' (see ./install --help)" "$rc|$out"

# --- t init in a terminal, with Obsidian vaults (tests/init.rs has the cases without one) ---
# A throwaway home with Obsidian's list of vaults; TODO_FILE unset, as for a new user.
vhome() { # <vault dir names...>: new $home listing these vaults (the last one most recently opened)
  home=$(mktemp -d); dirs+=$home
  local reg="$home/Library/Application Support/obsidian" json= i=0 v
  mkdir -p "$reg"
  for v; do mkdir -p "$home/Vaults/$v"; json+="${json:+,}\"v$i\":{\"path\":\"$home/Vaults/${v//\"/\\\"}\",\"ts\":$(( ++i ))}"; done
  print -r -- "{\"vaults\":{$json,\"gone\":{\"path\":\"$home/gone\",\"ts\":99}}}" >"$reg/obsidian.json"
}
tinit() { # <args...>: run t as the new user, without a terminal; sets $out and $rc
  out=$(unset TODO_FILE; cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData "$T_BIN" "$@" </dev/null 2>&1); rc=$?
}
ttyinit() { # <answers> <args...>: the same in a terminal, typing the answers; sets $out and $rc
  local ans=$1 buf= chunk i; shift; targs=("$@")
  zpty -b TTY 'stty -echo; print READY; (unset TODO_FILE; cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData "$T_BIN" "${targs[@]}"); print -r -- "<rc=$?>"'
  for i in {1..200}; do while zpty -r -t TTY chunk 2>/dev/null; do buf+=$chunk; done; [[ $buf == *READY* ]] && break; zselect -t 5; done
  buf=${buf#*READY$'\r\n'}
  zpty -w TTY "$ans"
  for i in {1..400}; do while zpty -r -t TTY chunk 2>/dev/null; do buf+=$chunk; done; [[ $buf == *'<rc='<->'>'* ]] && break; zselect -t 5; done
  zpty -d TTY
  buf=${buf//$'\r'/}; rc=${${buf##*<rc=}%%>*}; out=${${buf%<rc=*}%$'\n'}
}
# Where t init remembers the list: the config file.
#   cfgname: the file as messages show it; cfgfile: its path in $home;
#   cfgline <path under ~>: the line t shows and writes there.
cfgname='~/.config/markdone/config'
cfgfile() { print -r -- "$home/.config/markdone/config"; }
cfgline() { print -r -- "TODO_FILE=~/$1"; }
cfgsaved() { cfgline "$1"; }
extra='# my own line'

vhome Work Notes
ttyinit $'2\ny' init --obsidian
check "init --obsidian: lists vaults, newest first, and creates the list in Todo/" "0|Obsidian vaults:
  1  $home/Vaults/Notes
  2  $home/Vaults/Work
Which one should hold the list? [1] Created $home/Vaults/Work/Todo/Todo.md
Use this list from now on, also from AI agents? This adds TODO_FILE to $cfgname. [Y/n] Saved in $cfgname: t now uses $home/Vaults/Work/Todo/Todo.md" "$rc|$out"
check "init --obsidian: the list has the three headings" "$EMPTY" "$(<"$home/Vaults/Work/Todo/Todo.md")"$'\n'
check "init --obsidian: $cfgname gets one line" "$(cfgsaved Vaults/Work/Todo/Todo.md)" "$(<"$(cfgfile)")"
tinit add "from the vault"
check "after init --obsidian, t uses the vault's list" "0|Added 1 (Today): from the vault" "$rc|$out"
print -r -- "$extra" >>"$(cfgfile)"
ttyinit $'\n' init --obsidian
check "init --obsidian again: Enter picks the newest vault and saves" "0|Saved in $cfgname: t now uses $home/Vaults/Notes/Todo/Todo.md" "$rc|${${(f)out}[-1]##*\] }"
check "init --obsidian again: the earlier line is replaced in place, the rest kept" "$(cfgsaved Vaults/Notes/Todo/Todo.md)
$extra" "$(<"$(cfgfile)")"
ttyinit 1 init --obsidian
check "init --obsidian: an existing list is used as it is" \
  "$home/Vaults/Notes/Todo/Todo.md already exists; using it as it is|$cfgname already points t at this list" \
  "${${(@f)out}[-2]##*\] }|${${(@f)out}[-1]}"

vhome Solo
ttyinit n init --obsidian
check "init --obsidian, one vault, not saved: shows the line" "0|Obsidian vault: $home/Vaults/Solo
Created $home/Vaults/Solo/Todo/Todo.md
Use this list from now on, also from AI agents? This adds TODO_FILE to $cfgname. [Y/n] Not saved. To use the list, add this to $cfgname:
  $(cfgline Vaults/Solo/Todo/Todo.md)" "$rc|$out"
[[ ! -e $(cfgfile) ]]; check "init --obsidian, not saved: no $cfgname written" 0 $?


# The config file has one TODO_FILE line: t init replaces it, keeping the other lines.
vhome Solo; mkdir -p "${$(cfgfile):h}"; print -r -- $'TODO_FILE=/elsewhere.md\nTODO_JIRA_KEYS=PROJ' >"$(cfgfile)"
ttyinit y init --obsidian
check "init --obsidian: an earlier TODO_FILE line in the config is replaced" \
  "0|Saved in $cfgname: t now uses $home/Vaults/Solo/Todo/Todo.md" "$rc|${${(f)out}[-1]##*\] }"
check "init --obsidian: the config keeps its other lines" "$(cfgsaved Vaults/Solo/Todo/Todo.md)
TODO_JIRA_KEYS=PROJ" "$(<"$(cfgfile)")"

# Values aren't quoted or expanded, so any vault name is written as it is.
vhome 'My "odd" vault'
ttyinit y init --obsidian
check "init --obsidian: odd vault names are written as they are" "$(cfgsaved 'Vaults/My "odd" vault/Todo/Todo.md')" "$(<"$(cfgfile)")"
tinit add "odd"; check "init --obsidian: t uses the list in the odd vault" "0|Added 1 (Today): odd" "$rc|$out"

vhome Work Notes
ttyinit 7 init --obsidian; check "init --obsidian: a choice out of range fails" "1|t: no choice '7'; nothing done" "$rc|${${(f)out}[-1]##*\] }"
ttyinit 0 init --obsidian; check "init --obsidian: 0 isn't a choice" 1 "$rc"
ttyinit $'\C-d' init --obsidian; check "init --obsidian: Ctrl-D, nothing done" "1|t: no answer; nothing done" "$rc|${${(f)out}[-1]}"
[[ ! -e $home/Vaults/Work/Todo && ! -e $home/Vaults/Notes/Todo ]]; check "init --obsidian: failed choices create nothing" 0 $?



# Plain t init: offers the vaults only in a terminal, and only when TODO_FILE isn't set.
vhome Solo
ttyinit $'y\ny' init
check "plain init in a terminal offers the vault" "0|Keep the list in your Obsidian vault, in $home/Vaults/Solo/Todo? [Y/n] Created $home/Vaults/Solo/Todo/Todo.md" "$rc|${(F)${(@f)out}[1]}"
[[ -f $home/Vaults/Solo/Todo/Todo.md && ! -e $home/todo && -f $(cfgfile) ]]; check "plain init in a terminal: list made in the vault, and saved" 0 $?
vhome Solo
ttyinit n init
check "plain init in a terminal, vault declined: the default place" "0|Keep the list in your Obsidian vault, in $home/Vaults/Solo/Todo? [Y/n] Created $home/todo/Todo.md" "$rc|$out"
vhome Work Notes
ttyinit 0 init
check "plain init in a terminal, several vaults: 0 keeps the default place" "Created $home/todo/Todo.md" "${${(@f)out}[-1]##*\] }"

# --- t use alone, in a terminal ---
fresh "$EMPTY"; printf '## Today\n- [ ] Ship it\n\n## Later\n\n## Done\n' >"$dir/Work.md"
zpty -b TU 'stty -echo; PATH="$fakebin:$PATH" PICK=Work "$T_BIN" use; print -r -- "<rc=$?>"'
buf=; for i in {1..400}; do while zpty -r -t TU chunk 2>/dev/null; do buf+=$chunk; done; [[ $buf == *'<rc='<->'>'* ]] && break; zselect -t 5; done
zpty -d TU; buf=${buf//$'\r'/}
check "use alone in a terminal picks a list with fzf" "Now using Work (1 open)|<rc=0>" "${(j:|:)${(f)buf}}"

print "$pass passed, $fail failed"
(( fail == 0 ))
