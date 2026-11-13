#!/usr/bin/env zsh
# Runs bin/t against throwaway files and checks output, exit codes and file contents.
# Usage: test/run.zsh   (exits non-zero if any check fails)

T_BIN="${0:A:h:h}/bin/t"
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
dirs=(); trap 'chmod -R u+w $dirs 2>/dev/null; rm -rf $dirs' EXIT
# bin/t runs under zsh, which reads $ZDOTDIR/.zshenv (default ~/.zshenv) on every start. Point it at
# an empty dir so the user's TODO_* settings can't override the test's, or send t to the real list.
export ZDOTDIR=$(mktemp -d); dirs+=$ZDOTDIR
fresh() { dir=$(mktemp -d); dirs+=$dir; export TODO_FILE="$dir/Todo.md"; print -rn -- "$1" >"$TODO_FILE"; }
file() { cat "$TODO_FILE"; }
EMPTY=$'## Today\n\n## Later\n\n## Done\n'

# --- adding and listing ---
fresh "$EMPTY"
run;                          check "list empty"            "" "$out"
run add first;                check "add"                   "Added 1 (Today): first" "$out"
run add --later "some day";   check "add --later"           "Added 2 (Later): some day" "$out"
run a -l short;               check "alias a -l"            "Added 3 (Later): short" "$out"
run a second;                 check "add goes to end of Today, numbered" "Added 2 (Today): second" "$out"
weird='Ticket & "quotes" back\slash \n $HOME `x`'
run add "$weird";             check "special characters echoed as typed" "Added 3 (Today): $weird" "$out"
run list
check "list" "  1  Today  first
  2  Today  second
  3  Today  $weird
  4  Later  some day
  5  Later  short" "$out"
run ls; out2=$out; run l;     check "list aliases" "$out2" "$out"

# --- done ---
run done 2;                   check "done"                  "Done: second" "$out"
run d 2;                      check "alias d"               "Done: $weird" "$out"
check "file after adds and dones" "## Today
- [ ] first

## Later
- [ ] some day
- [ ] short

## Done
- [x] $today $weird
- [x] $today second
" "$(file)"$'\n'

# --- errors leave the file untouched ---
before=$(file)
for args in "add" "add --later" "done" "done x" "done 99" "bogus"; do
  run ${=args}; check "'t $args' fails" 1 "$rc"
done
run add "$(printf 'two\nlines')"; check "multi-line add fails" 1 "$rc"
check "file unchanged after errors" "$before" "$(file)"
run help;  check "help exits 0" 0 "$rc"; [[ $out == *"t add [--later] <text>"* ]]; check "help text" 0 $?
run --help; check "--help exits 0" 0 "$rc"

# --- tidy: dating, moving and archiving ---
fresh "## Today
- [ ] Open one
- [x] Ticked in an editor

## Later
- [x] $d8 Old ticked later

## Done
- [x] $d7 Seven days
- [x] Undated
- [x] $d8 Eight days
- [x] 2020-05-01 Ancient
"
y=${d8[1,4]}
printf '# Archive %s\n\n- [x] %s-01-01 Existing\n' "$y" "$y" >"$dir/Archive $y.md"
run
check "tidy list output" "  1  Today  Open one" "$out"
check "tidy list file" "## Today
- [ ] Open one

## Later

## Done
- [x] $today Ticked in an editor
- [x] $d7 Seven days
- [x] $today Undated" "$(file)"
check "archive appended and sorted" "# Archive $y

- [x] $y-01-01 Existing
- [x] $d8 Eight days
- [x] $d8 Old ticked later" "$(<"$dir/Archive $y.md")"
check "archive per year" "# Archive 2020

- [x] 2020-05-01 Ancient" "$(<"$dir/Archive 2020.md")"
before=$(file); run; check "second run changes nothing" "$before" "$(file)"

# a failed archive write keeps the item in the list
fresh "## Today
- [x] Ticked

## Done
- [x] 2020-06-01 Old item
"
printf '# Archive 2020\n\n' >"$dir/Archive 2020.md"; chmod a-w "$dir/Archive 2020.md"
before=$(file); run
[[ $out == *"t: could not write $dir/Archive 2020.md; list not tidied"* ]]
check "archive write failure: warns" 0 $?
check "archive write failure: list untouched" "$before" "$(file)"
check "archive write failure: archive untouched" $'# Archive 2020\n' "$(<"$dir/Archive 2020.md")"$'\n'
chmod u+w "$dir/Archive 2020.md"
run; check "archives once writable again" "- [x] 2020-06-01 Old item" "$(grep Old "$dir/Archive 2020.md")"

# --- today: move an item from Later to Today ---
fresh "## Today
- [ ] A

## Later
- [ ] B
- [ ] C PROJ-5

## Done
"
run today 3
check "today moves item to the end of Today" "Moved to Today as 2: C [PROJ-5](https://jira.example.com/browse/PROJ-5)" "$out"
run promote 3
check "promote alias; numbers come from the current list" "Moved to Today as 3: B" "$out"
check "file after moving both" "## Today
- [ ] A
- [ ] C [PROJ-5](https://jira.example.com/browse/PROJ-5)
- [ ] B

## Later

## Done" "$(file)"
before=$(file)
run today 1;  check "item already in Today fails" "t: item 1 is already in Today" "$out"
run today 9;  check "today with no such item fails" 1 "$rc"
run today x;  check "today with a non-number fails" 1 "$rc"
run today;    check "today without a number fails" 1 "$rc"
check "file unchanged after failed moves" "$before" "$(file)"
fresh $'## Later\n- [ ] B\n\n## Done\n'
run today 1; check "today without a Today heading fails" 1 "$rc"
check "item not lost when Today is missing" $'## Later\n- [ ] B\n\n## Done' "$(file)"

# --- later: move an item from Today to Later ---
fresh "## Today
- [ ] A
- [ ] B

## Later
- [ ] C

## Done
"
run later 1;  check "later moves item to the end of Later" "Moved to Later as 3: A" "$out"
run demote 1; check "demote alias; numbers come from the current list" "Moved to Later as 3: B" "$out"
check "file after moving both" "## Today

## Later
- [ ] C
- [ ] A
- [ ] B

## Done" "$(file)"
before=$(file)
run later 1;  check "item already in Later fails" "t: item 1 is already in Later" "$out"
run later 9;  check "later with no such item fails" 1 "$rc"
run later;    check "later without an item fails" "t: usage: t later <item>" "$out"
check "file unchanged after failed moves" "$before" "$(file)"
fresh $'## Today\n- [ ] A\n\n## Done\n'
run later 1; check "later without a Later heading fails" "t: no '## Later' heading in $TODO_FILE" "$out"
check "item not lost when Later is missing" $'## Today\n- [ ] A\n\n## Done' "$(file)"

# --- list --today / --later ---
fresh "## Today
- [ ] A
- [ ] B

## Later
- [ ] C

## Done
- [x] $today D
"
run list --today; check "list --today: only Today's items" "  1  Today  A
  2  Today  B" "$out"
run list --later; check "list --later: keeps the full-list number" "  3  Later  C" "$out"
full_list=$("$T_BIN" list)
run ls --later --today; check "--today --later together list everything" "$full_list" "$out"
run list --today --done; check "--today with --done fails" 1 "$rc"
run list --later --all;  check "--later with --all fails" 1 "$rc"
run today 3; run list --later; check "list --later when Later is empty" "" "$out"

# --- items by text ---
p7="[PROJ-7](https://jira.example.com/browse/PROJ-7)"
fresh "## Today
- [ ] Fix login bug
- [ ] Write docs (v2.1)

## Later
- [ ] Fix logout PROJ-7

## Done
"
run done write;          check "done by text" "Done: Write docs (v2.1)" "$out"
run undo
run done LOGIN BUG;      check "text: case-insensitive, words joined" "Done: Fix login bug" "$out"
run undo
run d "(v2";             check "text: matched literally, not as a pattern" "Done: Write docs (v2.1)" "$out"
run undo
run later docs;          check "later by text" "Moved to Later as 3: Write docs (v2.1)" "$out"
run today logout;        check "today by text" "Moved to Today as 2: Fix logout $p7" "$out"
run done PROJ-7;         check "a linked Jira key matches" "Done: Fix logout $p7" "$out"
run undo; run undo; run undo   # undo, redo, undo: back to before "done PROJ-7"
before=$(file)
run done fix
check "ambiguous text fails, listing the matches" "1|t: 'fix' matches 2 open items; use more of the text, or the number:
  1  Today  Fix login bug
  2  Today  Fix logout $p7" "$rc|$out"
run done "f.x";          check "no match fails" "1|t: no open item matches 'f.x'" "$rc|$out"
run done;                check "done without an item fails" "1|t: usage: t done <item>" "$rc|$out"
run today login;         check "today: text naming a Today item" "1|t: item 1 is already in Today" "$rc|$out"
check "file unchanged after failed references" "$before" "$(file)"

# --- rm ---
fresh "## Today
- [ ] keep
- [ ] old idea

## Later
- [ ] later idea

## Done
- [x] $today done item
"
run rm 2;          check "rm by number" "0|Removed: old idea" "$rc|$out"
run remove later;  check "alias remove, by text" "0|Removed: later idea" "$rc|$out"
check "rm leaves other lines alone" "## Today
- [ ] keep

## Later

## Done
- [x] $today done item" "$(file)"
run undo;          check "rm can be undone" "- [ ] later idea" "$(grep later "$TODO_FILE")"
before=$(file)
run rm 9;          check "rm with no such item fails" "1|t: no open item 9" "$rc|$out"
run rm "done item"; check "rm only takes open items" "1|t: no open item matches 'done item'" "$rc|$out"
run rm;            check "rm without an item fails" "1|t: usage: t rm <item>" "$rc|$out"
check "file unchanged after failed rm" "$before" "$(file)"

# --- reword ---
fresh "## Today
- [ ] first
- [ ] Retest logout

## Later
- [ ] third

## Done
"
run reword 2 "Retest logout fix PROJ-9"
check "reword by number, linking keys" "0|Reworded 2 (Today): Retest logout fix [PROJ-9](https://jira.example.com/browse/PROJ-9)" "$rc|$out"
run reword third Plan the next release
check "reword by text, new text from several words" "0|Reworded 3 (Later): Plan the next release" "$rc|$out"
check "reword keeps section and position" "## Today
- [ ] first
- [ ] Retest logout fix [PROJ-9](https://jira.example.com/browse/PROJ-9)

## Later
- [ ] Plan the next release

## Done" "$(file)"
run undo;  check "reword can be undone" "- [ ] third" "$(grep third "$TODO_FILE")"
before=$(file)
run reword first;   check "reword without new text fails" "1|t: usage: t reword <item> <new text>" "$rc|$out"
run reword;         check "reword without arguments fails" "1|t: usage: t reword <item> <new text>" "$rc|$out"
run reword 9 x;     check "reword with no such item fails" "1|t: no open item 9" "$rc|$out"
run reword 1 "$(printf 'two\nlines')"; check "reword to several lines fails" "1|t: item must be a single line" "$rc|$out"
check "file unchanged after failed rewords" "$before" "$(file)"

# --- listing done items ---
fresh "## Today
- [ ] Open one
- [x] Ticked in an editor

## Done
- [x] $today Alpha
- [x] $d7 Bravo
"
printf '# Archive %s\n\n- [x] %s Charlie\n' "$y" "$d8" >"$dir/Archive $y.md"
printf '# Archive 2020\n\n- [x] 2020-05-01 Ancient\n- [x] 2020-06-01 Later ancient\n' >"$dir/Archive 2020.md"
printf -- '- [x] 2099-01-01 Not an archive\n' >"$dir/Archive notes.md"
run list --done
check "list --done: Done section, newest first, editor ticks included" "$today  Ticked in an editor
$today  Alpha
$d7  Bravo" "$out"
run list --done --all
check "list --done --all: includes year archives only" "$today  Ticked in an editor
$today  Alpha
$d7  Bravo
$d8  Charlie
2020-06-01  Later ancient
2020-05-01  Ancient" "$out"
expected=$out; run ls --all --done; check "option order and alias" "$expected" "$out"
run list --all;   check "--all without --done fails" 1 "$rc"
run list --bogus; check "unknown list option fails" 1 "$rc"
fresh "$EMPTY"; run list --done; check "list --done with nothing done" "" "$out"

# --- Jira links ---
L() { print -r -- "[$1](https://jira.example.com/browse/$1)"; }
fresh "$EMPTY"
run add "Retest PROJ-1877 and OPS-12."
check "add links Jira keys" "Added 1 (Today): Retest $(L PROJ-1877) and $(L OPS-12)." "$out"
plain='Leave UTF-8, ISO-8601, ABC-12, xPROJ-4, PROJ-5a, `PROJ-3`, https://example.com/PROJ-2 and [see PROJ-6](https://example.com) alone'
run add "$plain"
check "non-keys, unknown projects, code, URLs and existing links untouched" "Added 2 (Today): $plain" "$out"
fresh "## Today
- [ ] Typed in an editor PROJ-10
Notes about PROJ-11

## Done
- [x] $today Shipped WEB-7
"
run
check "list prints raw Markdown when not a terminal" "  1  Today  Typed in an editor $(L PROJ-10)" "$out"
check "keys typed in an editor are linked; non-item lines untouched" "## Today
- [ ] Typed in an editor $(L PROJ-10)
Notes about PROJ-11

## Done
- [x] $today Shipped $(L WEB-7)" "$(file)"
before=$(file); run; check "linking is idempotent" "$before" "$(file)"
on_tty() { python3 -I -c 'import pty,sys; pty.spawn(sys.argv[1:])' "$T_BIN" "$@" </dev/null | tr -d '\r'; }  # run t with a pseudo-terminal as stdout
osc() { print -rn -- $'\e]8;;'"https://jira.example.com/browse/$1"$'\e\\'"$1"$'\e]8;;\e\\'; }
out=$(on_tty list)
check "list on a terminal shows clickable keys" "  1  Today  Typed in an editor $(osc PROJ-10)" "$out"
out=$(on_tty list --done)
check "list --done on a terminal shows clickable keys" "$today  Shipped $(osc WEB-7)" "$out"

# linking off: empty, blank or unset TODO_JIRA_KEYS, silently
off='Fix offset -42 for PROJ-1 and OPS-2'
for setting in "TODO_JIRA_KEYS=" "TODO_JIRA_KEYS= " "-u TODO_JIRA_KEYS"; do
  fresh "$EMPTY"
  out=$(env ${=setting} "$T_BIN" add "$off" 2>&1)
  check "no linking with env $setting" "Added 1 (Today): $off" "$out"
done
fresh "## Today
- [ ] Linked $(L PROJ-1) and bare PROJ-2

## Done
"
before=$(file); out=$(TODO_JIRA_KEYS= "$T_BIN" 2>&1)
check "tidy with linking off leaves bare keys and links alone" "$before" "$(file)"

# wrong Jira settings: warn, turn linking off, never write broken links
jira_bad() { # <name> <expected warning> <env assignments...>
  local name=$1 warning=$2; shift 2
  fresh "$EMPTY"
  out=$(env "$@" "$T_BIN" add "$off" 2>&1)
  check "$name" "t: $warning
Added 1 (Today): $off" "$out"
}
jira_bad "URL without trailing slash" \
  "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'https://x.example/browse'); Jira linking is off" \
  TODO_JIRA_URL=https://x.example/browse
jira_bad "URL without scheme" \
  "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'x.example/browse/'); Jira linking is off" \
  TODO_JIRA_URL=x.example/browse/
jira_bad "keys set but no URL" "TODO_JIRA_KEYS is set but TODO_JIRA_URL isn't; Jira linking is off" TODO_JIRA_URL=
jira_bad "comma-separated keys" \
  "TODO_JIRA_KEYS: 'PROJ,OPS' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off" \
  "TODO_JIRA_KEYS=PROJ,OPS WEB"
jira_bad "lowercase and regex keys" \
  "TODO_JIRA_KEYS: 'proj', 'A.B' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off" \
  "TODO_JIRA_KEYS=proj A.B"
fresh "$EMPTY"
out=$(TODO_JIRA_KEYS="  PROJ   P2_X  " "$T_BIN" add "PROJ-1 P2_X-2 PROJX-3" 2>&1)
check "keys with digits/underscores, extra spaces" "Added 1 (Today): $(L PROJ-1) $(L P2_X-2) PROJX-3" "$out"

# perl failing while linking is on: stop with an error, never touch the list
noperl=$(mktemp -d); dirs+=$noperl
printf '#!/bin/sh\nexit 127\n' >"$noperl/perl"; chmod +x "$noperl/perl"
fresh "## Today
- [ ] keep me PROJ-1

## Done
"
before=$(file)
out=$(PATH="$noperl:$PATH" "$T_BIN" list 2>&1); rc=$?
check "perl failure: list fails" "1|t: Jira linking failed (is perl installed?); list not changed" "$rc|$out"
out=$(PATH="$noperl:$PATH" "$T_BIN" add new 2>&1); rc=$?
check "perl failure: add fails" 1 "$rc"
check "perl failure: list file untouched" "$before" "$(file)"
out=$(PATH="$noperl:$PATH" TODO_JIRA_KEYS= "$T_BIN" add new 2>&1)
check "perl not needed with linking off" "Added 2 (Today): new" "$out"

# --- t commands (used by Tab completion) ---
export TODO_FILE="/nonexistent/Todo.md"   # must work without a list
run commands
check "commands exits 0 without a list" 0 "$rc"
check "commands: one tab-separated line per command" 17 "$(print -r -- "$out" | grep -c $'^[^\t]*\t[^\t]* e\\.g\\. t ')"
[[ $out == *$'add ""\tadd "<text>"'*'Add an item to Today'*'e.g. t add "Retest logout fix PROJ-123"'* ]]
check "commands: add line has insert text, description and example" 0 $?
# every runnable insert (no placeholder) must be accepted by t
cmds=$out
fresh "$EMPTY"
for ins in ${(f)"$(print -r -- "$cmds" | cut -f1 | grep -v -e '""$' -e ' $' -e '^pick$' -e '^edit$' -e '^init$' -e '^undo$')"}; do
  run ${=ins}; check "inserted command 't $ins' runs" 0 "$rc"
done

# --- Tab completion widget (shell/t.zsh), driven through a pseudo-terminal ---
zmodload zsh/zpty zsh/zselect
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
setup "source '${T_BIN:h:h}/shell/t.zsh'"
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
zpty -d TT

# --- configuration from ~/.zshenv, in the form README.md recommends ---
zdot=$(mktemp -d); dirs+=$zdot
fresh "$EMPTY"; other=$TODO_FILE
print -r -- 'export TODO_FILE="${TODO_FILE:-'"$other"'}"' >"$zdot/.zshenv"
fresh "$EMPTY"
out=$(ZDOTDIR=$zdot "$T_BIN" add mine 2>&1)
check "a TODO_FILE given by the caller wins over ~/.zshenv" "Added 1 (Today): mine" "$out"
check "~/.zshenv's TODO_FILE untouched" "$EMPTY" "$(<"$other")"$'\n'
out=$(unset TODO_FILE; ZDOTDIR=$zdot "$T_BIN" add theirs 2>&1)
check "~/.zshenv fills in TODO_FILE when unset" "Added 1 (Today): theirs" "$out"

# --- edit ---
fresh "$EMPTY"
out=$(EDITOR="echo opened" "$T_BIN" edit 2>&1)
check "EDITOR with arguments" "opened $TODO_FILE" "$out"

# --- locking ---
lock_of() { print -r -- "${TODO_FILE:h}/.${TODO_FILE:t}.lock"; }
hold() { # <hundredths of a second>: hold the list's lock from another process; sets $holder
  local ready="${TODO_FILE:h}/held"
  zsh -fc 'zmodload zsh/system zsh/zselect
    : >>"$1"; zsystem flock -f fd "$1" && : >"$2" && zselect -t $3' x "$(lock_of)" "$ready" $1 &
  holder=$!
  until [[ -e $ready ]]; do zselect -t 1; done
  rm -f "$ready"
}
fresh "$EMPTY"
for i in {1..20}; do "$T_BIN" add "parallel $i" >/dev/null 2>&1 & done; wait
check "parallel adds: none lost" 20 "$(grep -c '^- \[ \] parallel ' "$TODO_FILE")"

fresh "$EMPTY"
hold 50
run add waited; check "waits for a lock that is released" "0|Added 1 (Today): waited" "$rc|$out"
wait $holder
hold 1500
before=$(file); run add blocked
check "gives up on a lock that is held" \
  "1|t: another t has been using the list for 10 seconds; try again when it's done" "$rc|$out"
check "held lock: list untouched" "$before" "$(file)"
kill -9 $holder; wait $holder 2>/dev/null
run add freed; check "lock of a killed process is free at once" "0|Added 2 (Today): freed" "$rc|$out"

# t edit execs the editor, which closes the lock's descriptor. The editor checks from a child
# process: the editor process itself (t after exec) could always relock a lock it still held.
fresh "$EMPTY"
cat >"$dir/editor" <<'EOS'
#!/bin/sh
zsh -fc 'zmodload zsh/system; zsystem flock -t 0 -f fd "$1" 2>/dev/null && echo free || echo held' x \
  "$(dirname "$1")/.$(basename "$1").lock"
EOS
chmod +x "$dir/editor"
out=$(EDITOR="$dir/editor" "$T_BIN" edit 2>&1)
check "the editor started by t edit doesn't hold the lock" free "$out"

fresh "$EMPTY"; chmod a-w "${TODO_FILE:h}"
run; check "unwritable directory: can't lock" "1|t: could not create the lock file $(lock_of)" "$rc|$out"
chmod u+w "${TODO_FILE:h}"
run help; [[ ! -e $(lock_of) ]]; check "help doesn't lock" 0 $?

# --- backup ---
bak_of() { print -r -- "${TODO_FILE:h}/.${TODO_FILE:t}.bak"; }
fresh "$EMPTY"
run list; [[ ! -e $(bak_of) ]]; check "no backup when nothing is written" 0 $?
run add one; check "backup holds the list before the write" "$EMPTY" "$(cat "$(bak_of)")"$'\n'
before=$(file); run add two
check "backup follows each write" "$before" "$(cat "$(bak_of)")"
[[ $(ls -l "$(bak_of)") == -rw-------* ]]; check "backup is private to the owner" 0 $?
before=$(file); chmod a-w "$(bak_of)"; run add three
check "backup fails: nothing written" "1|t: could not write the backup $(bak_of); list not changed" \
  "$rc|${out##*$'\n'}"
check "backup fails: list untouched" "$before" "$(file)"
chmod u+w "$(bak_of)"

# --- undo ---
fresh "$EMPTY"
run undo; check "undo with nothing to undo" "1|t: nothing to undo" "$rc|$out"
run add one; after_add=$(file)
run undo; check "undo" "0|Undid the last change; run t undo again to redo it" "$rc|$out"
check "undo restores the list" "$EMPTY" "$(file)"$'\n'
run undo; check "second undo redoes" "$after_add" "$(file)"
run done 1; run undo
check "undo a done item" "$after_add" "$(file)"
run later 1; run today 1; run undo
check "undo only reverts the last change" $'## Today\n\n## Later\n- [ ] one\n\n## Done' "$(file)"

# the list changed since t's last write: undo would throw that change away
fresh "$EMPTY"
run add one; print -r -- "- [ ] typed by hand" >>"$TODO_FILE"; touch -t 202001010000 "$TODO_FILE"
before=$(file); run undo
check "undo refuses after a hand edit" \
  "1|t: the list was changed since t last wrote it; not undoing (t's previous version is in $(bak_of))" "$rc|$out"
check "refused undo leaves the list alone" "$before" "$(file)"

# undo must not tidy first: that would be a change of its own
fresh "$EMPTY"
run add one; run add two
print -r -- "- [x] ticked by hand" >>"$TODO_FILE"; touch -r "$TODO_FILE" "$(bak_of)"
run undo; check "undo doesn't tidy first" $'## Today\n- [ ] one\n\n## Later\n\n## Done' "$(file)"

# undoing a tidy that archived would archive the same items twice
fresh "$EMPTY"
run add new; print -r -- "- [x] $d8 old" >>"$TODO_FILE"
run list   # tidy archives "old": the last write
check "tidy that archived: undo refuses" "1|t: nothing to undo" "$(run undo; print -r -- "$rc|$out")"
check "tidy that archived: archived once" 1 "$(grep -c 'old' "$dir/Archive ${d8[1,4]}.md")"

# --- edge cases ---
fresh $'## Today\n- [x] orphan\n'
run
check "ticked item with no Done heading: warns" "t: ticked items found but no '## Done' heading after them; list not tidied" "$out"
check "ticked item with no Done heading: file untouched" $'## Today\n- [x] orphan' "$(file)"

fresh $'## Today\n\n## Done\n\n## Later'
run add -l x; check "add to last section without trailing newline" "Added 1 (Later): x" "$out"
check "last section file" $'## Today\n\n## Done\n\n## Later\n- [ ] x' "$(file)"

fresh $'## Later\n\n## Done\n'
run add x; check "add without a Today heading fails" 1 "$rc"

export TODO_FILE="/nonexistent/Todo.md"
run; check "missing file fails and points to t init" "1|t: /nonexistent/Todo.md not found; create it with 't init', or set TODO_FILE to your list" "$rc|$out"

# --- t init ---
dir=$(mktemp -d); dirs+=$dir; export TODO_FILE="$dir/new dir/sub/Todo.md"
run init;  check "init creates the list and its directories" "0|Created $TODO_FILE" "$rc|$out"
check "init writes the three headings" "$EMPTY" "$(file)"$'\n'
run add first; check "list made by init is usable" "Added 1 (Today): first" "$out"
before=$(file)
run init;  check "init refuses an existing list" "1|t: $TODO_FILE already exists; not changed" "$rc|$out"
check "init leaves an existing list untouched" "$before" "$(file)"
ln -s "$dir/missing-target.md" "$dir/dangling.md"; export TODO_FILE="$dir/dangling.md"
run init;  check "init refuses a dangling symlink" 1 "$rc"
[[ ! -e $dir/missing-target.md ]]; check "init does not write through a dangling symlink" 0 $?
ro=$(mktemp -d); dirs+=$ro; chmod a-w "$ro"; export TODO_FILE="$ro/sub/Todo.md"
run init;  check "init fails when the directory can't be created" 1 "$rc"
[[ $out == *"t: could not create the directory $ro/sub" ]]; check "init failure names the directory" 0 $?

print "$pass passed, $fail failed"
(( fail == 0 ))
