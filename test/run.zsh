#!/usr/bin/env zsh
# Runs t against throwaway files and checks output, exit codes and file contents.
# Usage: test/run.zsh   (exits non-zero if any check fails)
#   T_BIN=/abs/path/to/t test/run.zsh   tests another build of t (default: bin/t, the zsh version)

repo=${0:A:h:h}
T_BIN=${T_BIN:-$repo/bin/t}
[[ $T_BIN == /* ]] || { print -r -- "T_BIN must be an absolute path (got '$T_BIN')" >&2; exit 2; }
[[ -x $T_BIN ]] || { print -r -- "T_BIN isn't an executable file: $T_BIN" >&2; exit 2; }
# Which implementation is under test: the zsh script or the Rust program (docs/designs/rust_port.md).
# Checks of behaviour only one of them has run only for that one.
if [[ $T_BIN == $repo/bin/t ]]; then impl=zsh; else impl=rust; fi
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
# A throwaway home as well, for everything this script and t run. Without it, a test that forgets to
# set HOME reaches the real one: t init reads Obsidian's list of vaults there and can create a list
# in a real vault and write ~/.zshenv; ./install links into ~/.local/bin and edits ~/.zshrc.
real_home=$HOME
export HOME=$(mktemp -d); dirs+=$HOME
# The Rust t reads a config file from $XDG_CONFIG_HOME (or %APPDATA% on Windows) and t init can write
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
export TODO_FILE="/nonexistent/Todo.md"   # version must work without a list
run version;   [[ $out == "t "<->.<->.<-> ]]; check "version: t <major>.<minor>.<patch>, exit 0" "0|0" "$?|$rc"
v=$out; run --version; check "--version" "$v" "$out"

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
check "archive appended and sorted (an old-style archive is moved to Archive/ first)" "# Archive $y

- [x] $y-01-01 Existing
- [x] $d8 Eight days
- [x] $d8 Old ticked later" "$(<"$dir/Archive/Todo $y.md")"
[[ ! -e "$dir/Archive $y.md" ]]; check "the old-style archive is gone" 0 $?
check "archive per year, per list" "# Todo archive 2020

- [x] 2020-05-01 Ancient" "$(<"$dir/Archive/Todo 2020.md")"
before=$(file); run; check "second run changes nothing" "$before" "$(file)"

# a failed archive write keeps the item in the list
fresh "## Today
- [x] Ticked

## Done
- [x] 2020-06-01 Old item
"
mkdir "$dir/Archive"; printf '# Todo archive 2020\n\n' >"$dir/Archive/Todo 2020.md"; chmod a-w "$dir/Archive/Todo 2020.md"
before=$(file); run
[[ $out == *"t: could not write $dir/Archive/Todo 2020.md; list not tidied"* ]]
check "archive write failure: warns" 0 $?
check "archive write failure: list untouched" "$before" "$(file)"
check "archive write failure: archive untouched" $'# Todo archive 2020\n' "$(<"$dir/Archive/Todo 2020.md")"$'\n'
chmod u+w "$dir/Archive/Todo 2020.md"
run; check "archives once writable again" "- [x] 2020-06-01 Old item" "$(grep Old "$dir/Archive/Todo 2020.md")"

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

# Text is matched against what t list shows in a terminal: links count by their text, not their URL.
fresh "## Today
- [ ] Fix logout PROJ-7
- [ ] Read [the guide](https://docs.example.com/guide) on https://example.org/faq
- [ ] Ask about [local notes](notes.md)

## Done
"
run done jira.example;   check "a Jira link's URL doesn't match" "1|t: no open item matches 'jira.example'" "$rc|$out"
run done docs.example;   check "a link's URL doesn't match" "1|t: no open item matches 'docs.example'" "$rc|$out"
run done browse;         check "no match in any link's URL" "1|t: no open item matches 'browse'" "$rc|$out"
run done "logout proj-7"; check "text across a link boundary matches" "Done: Fix logout $p7" "$out"
run undo
run done "the guide on";  check "a link matches by its text" 0 "$rc"
run undo
run done example.org;     check "a bare URL is shown, so it matches" 0 "$rc"
run undo
run done notes.md;        check "a non-web link is shown as is, so it matches" 0 "$rc"

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

# --- list --json ---
fresh "$EMPTY"
run list --json;   check "json: empty list" "0|[]" "$rc|$out"
fresh "## Today
- [ ] Say \"hi\" \\ back"$'\t'"slash PROJ-7, again PROJ-7, and OPS-1

## Later
- [ ] Later one

## Done
- [x] $today done
"
jsonok() { python3 -I -c 'import json,sys; json.load(sys.stdin)' <<<"$1"; }
run list --json
check "json: one object per item" '[
{"list":"Todo","n":1,"section":"Today","text":"Say \"hi\" \\ back\u0009slash [PROJ-7](https://jira.example.com/browse/PROJ-7), again [PROJ-7](https://jira.example.com/browse/PROJ-7), and [OPS-1](https://jira.example.com/browse/OPS-1)","jira":["PROJ-7","OPS-1"]},
{"list":"Todo","n":2,"section":"Later","text":"Later one","jira":[]}
]' "$out"
jsonok "$out"; check "json: parses as JSON" 0 $?
run list --later --json
check "json: --later keeps the numbers" '[
{"list":"Todo","n":2,"section":"Later","text":"Later one","jira":[]}
]' "$out"
out=$(TODO_JIRA_KEYS= "$T_BIN" list --today --json); jsonok "$out"
check "json: no jira keys with linking off" "0|1" "$?|$(grep -c '"jira":\[\]' <<<"$out")"
out=$(python3 -I -c 'import pty,sys; pty.spawn(sys.argv[1:])' "$T_BIN" list --json </dev/null)
[[ $out == "["* && $out != *$'\e'* ]]; check "json: plain even on a terminal" 0 $?
run list --done --json
check "json: not with --done" "1|t: --json lists open items; it doesn't combine with --done" "$rc|$out"

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
check "commands: one tab-separated line per command" 27 "$(print -r -- "$out" | grep -c $'^[^\t]*\t[^\t]* e\\.g\\. t ')"
[[ $out == *$'add ""\tadd "<text>"'*'Add an item to Today'*'e.g. t add "Retest logout fix PROJ-123"'* ]]
check "commands: add line has insert text, description and example" 0 $?
# every runnable insert (no placeholder) must be accepted by t
cmds=$out
fresh "$EMPTY"
for ins in ${(f)"$(print -r -- "$cmds" | cut -f1 | grep -v -e '""$' -e ' $' -e '^pick$' -e '^edit$' -e '^init' -e '^undo$' -e '^@$')"}; do
  run ${=ins}; check "inserted command 't $ins' runs" 0 "$rc"
done

# --- t complete (used by Tab completion for items) ---
fresh "## Today
- [ ] Fix login
- [ ] Fix logout PROJ-7

## Later
- [ ] Clean up

## Done
"
run complete done
check "complete done: every open item, links shown as their text" \
  $'0|1\t  1  Today  Fix login\n2\t  2  Today  Fix logout PROJ-7\n3\t  3  Later  Clean up' "$rc|$out"
run complete today;   check "complete today: Later's items"  $'3\t  3  Later  Clean up' "$out"
run complete demote;  check "complete demote (later): Today's items" $'1\t  1  Today  Fix login\n2\t  2  Today  Fix logout PROJ-7' "$out"
run complete reword;  check "complete reword: cursor goes between quotes" $'1 ""\t  1  Today  Fix login' "${out%%$'\n'*}"
run complete d;       check "complete: aliases work" "$(run complete done; print -r -- "$out")" "$out"
export TODO_FILE="/nonexistent/Todo.md"
run complete add;     check "complete: no items for a command without one, list not read" "0|" "$rc|$out"
run complete;         check "complete without a command: nothing" "0|" "$rc|$out"

# --- several lists ---
fresh "$EMPTY"; L1=$dir
run lists;               check "lists: just the default list, current" "0|* Todo   0 open" "$rc|$out"
run lists add Work;      check "lists add" "0|Created the list Work; use it with t @Work … or t use Work" "$rc|$out"
check "lists add writes the three headings" "$EMPTY" "$(<"$L1/Work.md")"$'\n'
run lists add work;      check "lists add: names are unique, ignoring case" "1|t: there's already a list called Work" "$rc|$out"
for bad in a/b .hidden @at x.md "Archive 2020" ""; do
  run lists add "$bad"; check "lists add refuses the name '$bad'" 1 "$rc"
done
run lists add "Side projects"; check "lists add: a name with a space" 0 "$rc"
run @work add "Fix the build"
check "@list (any case) works on that list" "0|Added 1 (Today): Fix the build" "$rc|$out"
check "@list: the default list is untouched" "$EMPTY" "$(file)"$'\n'
run @"Side projects" add -l "Write the post"; check "@list with a space" "Added 1 (Later): Write the post" "$out"
run lists
check "lists: counts, sorted, current marked" "  Side projects   1 open
* Todo            0 open
  Work            1 open" "$out"
run @Nope list;          check "@list that doesn't exist" "1|t: no list called 'Nope'; t lists shows them" "$rc|$out"
run @ list;              check "@ without a name" 1 "$rc"
run @Work lists;         check "@list with t lists fails" "1|t: t lists works on the lists themselves; it doesn't take @Work" "$rc|$out"
run @Work init;          check "@list with t init fails" 1 "$rc"

run use work;            check "use switches the current list" "0|Now using Work (1 open)" "$rc|$out"
check "use writes the name to .current" "Work" "$(<"$L1/.current")"
run add "Second";        check "after use, commands go to that list" "Added 2 (Today): Second" "$out"
run list;                check "after use, list shows that list" "  1  Today  Fix the build
  2  Today  Second" "$out"
run lists;               check "lists marks the current list" "* Work            2 open" "${${(f)out}[3]}"
out=$(TODO_LIST=Todo "$T_BIN" list --json 2>&1)
check "TODO_LIST wins over the current list" "[]" "$out"
out=$(TODO_LIST=Nope "$T_BIN" list 2>&1); rc=$?
check "TODO_LIST naming no list fails" "1|t: TODO_LIST is 'Nope', but there's no such list; t lists shows them" "$rc|$out"
out=$(TODO_LIST=Todo "$T_BIN" use Work 2>&1)
check "use warns that TODO_LIST still wins" "Now using Work (2 open)
t: TODO_LIST is set to 'Todo', which still wins where it's set" "$out"
run use Nope;            check "use with an unknown list" "1|t: no list called 'Nope'; t lists shows them" "$rc|$out"
run use;                 check "use alone without a terminal fails" 1 "$rc"
run @Work list --json
check "list --json names the list" '[
{"list":"Work","n":1,"section":"Today","text":"Fix the build","jira":[]},
{"list":"Work","n":2,"section":"Today","text":"Second","jira":[]}
]' "$out"
run lists --json
check "lists --json" '[
{"name":"Side projects","open":1,"current":false},
{"name":"Todo","open":0,"current":false},
{"name":"Work","open":2,"current":true}
]' "$out"
out=$(on_tty list); check "list on a terminal names the list when there are several" $'\e[1mWork\e[0m' "${${(f)out}[1]}"

# each list has its own undo, archives and lock
run @Todo add "Home thing"; run add "Third"; run @Todo undo
check "undo is per list: Todo's last change undone" "$EMPTY" "$(file)"$'\n'
run list;                check "undo is per list: Work kept its change" "3" "${#${(f)out}}"
print -r -- "- [x] $d8 Old work" >>"$L1/Work.md"; run list
[[ -f "$L1/Archive/Work ${d8[1,4]}.md" ]]; check "archives are per list, in Archive/" 0 $?
run @Todo list --done --all; check "list --done --all shows only that list's archives" "" "$out"
run list --done --all;   check "list --done --all includes the list's own archives" "$d8  Old work" "$out"

# the current list deleted by hand: back to the default list, with a warning
cp "$L1/Work.md" "$L1/work.keep"; rm "$L1/Work.md"
run list;                check "a vanished current list falls back to the default" "0|t: the current list 'Work' is gone; using Todo (t use picks another)" "$rc|$out"
mv "$L1/work.keep" "$L1/Work.md"
run use Todo;            check "use the default list" "Now using Todo (0 open)" "$out"
[[ ! -e $L1/.current ]]; check "using the default list clears .current" 0 $?

# rename and delete
run use Work
run lists rename Work Job; check "lists rename" "0|Renamed the list Work to Job" "$rc|$out"
[[ -f $L1/Job.md && ! -e $L1/Work.md && -f "$L1/Archive/Job ${d8[1,4]}.md" && ! -e "$L1/Archive/Work ${d8[1,4]}.md" ]]
check "lists rename moves the file and its archives" 0 $?
check "lists rename keeps a current list current" "Job" "$(<"$L1/.current")"
run lists rename Todo Home; check "lists rename refuses the default list" 1 "$rc"
run lists rename Job "Side projects"; check "lists rename refuses an existing name" "1|t: there's already a list called Side projects" "$rc|$out"
run lists rm Todo;       check "lists rm refuses the default list" "1|t: Todo is your default list (TODO_FILE); it can't be deleted" "$rc|$out"
run lists rm Job;        check "lists rm refuses the current list" "1|t: Job is the current list; switch with t use first" "$rc|$out"
run use Todo
run lists rm Job;        check "lists rm refuses a list with open items" "1|t: Job has 3 open items; finish or move them first, or use t lists rm --force Job" "$rc|$out"
run lists rm --force job; check "lists rm --force" "0|Deleted the list Job with its 3 open items; its archives stay in $L1/Archive" "$rc|$out"
[[ ! -e $L1/Job.md && ! -e $L1/.Job.md.bak && ! -e $L1/.Job.md.lock && -f "$L1/Archive/Job ${d8[1,4]}.md" ]]
check "lists rm deletes the file and its hidden files, keeps the archives" 0 $?
run @"Side projects" done post; run lists rm "Side projects"
check "lists rm of a list without open items" "0|Deleted the list Side projects; its archives stay in $L1/Archive" "$rc|$out"
run lists rm Nope;       check "lists rm of an unknown list" 1 "$rc"

# Tab completion data
run lists add Work; run @Work add "Ship it"
run complete @;          check "complete @: the lists, as @<name>" $'@Todo \t* Todo   0 open\n@Work \t  Work   1 open' "$out"
run complete use;        check "complete use: the lists" $'Todo \t* Todo   0 open\nWork \t  Work   1 open' "$out"
run complete lists rm;   check "complete lists rm: the lists" 2 "${#${(f)out}}"
run complete @Work done; check "complete @list done: that list's items" $'1\t  1  Today  Ship it' "$out"
run complete @Work;      check "complete @list: the commands" "$(run commands; print -r -- "$out")" "$out"
run complete lists;      check "complete lists: its subcommands" 'add ""|rm |rename |--json' "${(j:|:)${(@)${(f)out}%%$'\t'*}}"
run complete @Nope done; check "complete with an unknown list fails" 1 "$rc"

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
  out=$(PATH="$fakebin:$PATH" T_BIN="$T_BIN" T_LOCK="$(lock_of)" PICK="$1" PICK_DURING="$2" \
    "$T_BIN" pick 2>&1); rc=$?
}
ABC=$'## Today\n- [ ] A\n- [ ] B\n\n## Later\n- [ ] C\n\n## Done\n'
fresh "$ABC"
run_pick B; check "pick marks the chosen item done" "0|Done: B" "$rc|$out"

fresh "$ABC"
run_pick C 'zsh -fc '\''zmodload zsh/system; zsystem flock -t 0 -f fd "$1" && echo free || echo held'\'' x "$T_LOCK" >"$T_LOCK.state"; "$T_BIN" rm A'
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

# an edit within the same second keeps the mtime (1-second filesystems): only the content tells
fresh "$EMPTY"
run add one; print -r -- "- [ ] typed by hand" >>"$TODO_FILE"; touch -r "$(bak_of)" "$TODO_FILE"
before=$(file); run undo
check "undo refuses after a hand edit that kept the mtime" \
  "1|t: the list was changed since t last wrote it; not undoing (t's previous version is in $(bak_of))" "$rc|$out"
check "refused undo leaves the list alone (same mtime)" "$before" "$(file)"

# the checksum of t's last write is missing (written by t 0.1.0, or lost): don't guess
sum_of() { print -r -- "${TODO_FILE:h}/.${TODO_FILE:t}.sum"; }
fresh "$EMPTY"
run add one; [[ -s $(sum_of) ]]; check "each write records a checksum" 0 $?
rm "$(sum_of)"; run undo
check "undo without a checksum refuses" \
  "1|t: can't tell whether the list was changed since t last wrote it; not undoing (t's previous version is in $(bak_of))" "$rc|$out"
fresh "$EMPTY"
run add one; chmod a-w "$(sum_of)"; run add two
check "checksum write fails: the change is made, with a warning" \
  "0|t: could not write $(sum_of); t undo won't undo this change|Added 2 (Today): two" "$rc|${(j:|:)${(f)out}}"
run undo; check "checksum write failed: undo refuses" 1 "$rc"
check "checksum write failed: list kept" $'## Today\n- [ ] one\n- [ ] two\n\n## Later\n\n## Done' "$(file)"

# undo must not tidy first: that would be a change of its own. Here tidy would link PROJ-1, which
# t added while linking was off.
fresh "$EMPTY"
TODO_JIRA_KEYS= run add one; TODO_JIRA_KEYS= run add "two PROJ-1"
run undo; check "undo doesn't tidy first" $'## Today\n- [ ] one\n\n## Later\n\n## Done' "$(file)"

# undoing a tidy that archived would archive the same items twice
fresh "$EMPTY"
run add new; print -r -- "- [x] $d8 old" >>"$TODO_FILE"
run list   # tidy archives "old": the last write
check "tidy that archived: undo refuses" "1|t: nothing to undo" "$(run undo; print -r -- "$rc|$out")"
check "tidy that archived: archived once" 1 "$(grep -c 'old' "$dir/Archive/Todo ${d8[1,4]}.md")"

# --- the README demo stays true ---
# Replays every "$ t ..." line of the README's console block and compares the whole transcript.
demo=$(awk '/^```console$/{on=1; next} on && /^```$/{exit} on' "$repo/README.md")
fresh "$EMPTY"
replay=$(for cmd in ${(f)"$(grep '^\$ t ' <<<"$demo")"}; do
  print -r -- "$cmd"; TODO_JIRA_KEYS= eval "\"\$T_BIN\" ${cmd#\$ t }" 2>&1
done)
check "README demo matches real output" "$demo" "$replay"

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

# --- ./install ---
# Always with a throwaway home: ./install edits ~/.zshrc and links into ~/.local/bin.
inst() { out=$(cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData PATH="$home/.local/bin:$fakebin:$PATH" "$repo/install" "$@" 2>&1); rc=$?; }
home=$(mktemp -d); dirs+=$home
print -r -- 'source <(fzf --zsh)' >"$home/.zshrc"
inst
check "install: links t and turns on Tab completion" "0|Linked $home/.local/bin/t -> $repo/bin/t
Turned on Tab completion: added to the end of $home/.zshrc:
  source '$repo/shell/t.zsh'
Done. Open a new terminal, then run 't init' to create your list (or set TODO_FILE first)." "$rc|$out"
[[ ${${:-$home/.local/bin/t}:A} == $repo/bin/t ]]; check "install: the link points at this clone's t" 0 $?
check "install: ~/.zshrc gets one marked source line" "source <(fzf --zsh)

# markdone: Tab completion for t (remove with ./install --uninstall)
source '$repo/shell/t.zsh'" "$(<"$home/.zshrc")"
before=$(<"$home/.zshrc"); inst
check "install again: nothing to do" "0|t is already linked: $home/.local/bin/t
Tab completion is already on in $home/.zshrc
Done. Open a new terminal, then run 't init' to create your list (or set TODO_FILE first)." "$rc|$out"
check "install again: ~/.zshrc unchanged" "$before" "$(<"$home/.zshrc")"
inst --uninstall
check "uninstall removes the link and the line" "0|Removed $home/.local/bin/t
Removed Tab completion from $home/.zshrc" "$rc|$out"
check "uninstall leaves the rest of ~/.zshrc as it was" "source <(fzf --zsh)" "$(<"$home/.zshrc")"
inst --uninstall; check "uninstall again: nothing to do" "0|No link to this t at $home/.local/bin/t; left alone" "$rc|$out"

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
check "install --bin off PATH: links there, warns" "0|Linked $home/elsewhere/t -> $repo/bin/t|install: $home/elsewhere isn't on your PATH; add it, e.g. in ~/.zshenv: export PATH=\"$home/elsewhere:\$PATH\"" \
  "$rc|${(j:|:)${(@f)out}[1,2]}"
[[ $out == *"doesn't seem to load fzf's key bindings"* ]]; check "install warns when ~/.zshrc doesn't load fzf" 0 $?

home=$(mktemp -d); dirs+=$home
for c in mkdir ln sed; do ln -sf "${commands[$c]}" "$nofzf/$c"; done
out=$(cd /; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData PATH="$home/.local/bin:$nofzf" "$repo/install" 2>&1); rc=$?
check "install without fzf: Tab completion left off" "0|install: fzf isn't installed, so Tab completion is left off; install fzf and run ./install again" "$rc|${${(f)out}[2]}"
[[ ! -e $home/.zshrc ]]; check "install without fzf: no ~/.zshrc written" 0 $?

inst --help; [[ $rc == 0 && $out == *"./install --uninstall"* ]]; check "install --help" 0 $?
inst --bogus; check "install: unknown option fails" "1|install: unknown option '--bogus' (see ./install --help)" "$rc|$out"
# --- t init with Obsidian vaults ---
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
MARK='# markdone: where t keeps the list (set by t init)'

vhome Work Notes
ttyinit $'2\ny' init --obsidian
check "init --obsidian: lists vaults, newest first, and creates the list in Todo/" "0|Obsidian vaults:
  1  $home/Vaults/Notes
  2  $home/Vaults/Work
Which one should hold the list? [1] Created $home/Vaults/Work/Todo/Todo.md
Use this list from now on, also from AI agents? This adds TODO_FILE to ~/.zshenv. [Y/n] Saved in ~/.zshenv: t now uses $home/Vaults/Work/Todo/Todo.md" "$rc|$out"
check "init --obsidian: the list has the three headings" "$EMPTY" "$(<"$home/Vaults/Work/Todo/Todo.md")"$'\n'
check "init --obsidian: ~/.zshenv gets one marked line" "$MARK
export TODO_FILE=\"\${TODO_FILE:-\$HOME/Vaults/Work/Todo/Todo.md}\"" "$(<"$home/.zshenv")"
tinit add "from the vault"
check "after init --obsidian, t uses the vault's list" "0|Added 1 (Today): from the vault" "$rc|$out"
print -r -- 'export EDITOR=vi' >>"$home/.zshenv"
ttyinit $'\n' init --obsidian
check "init --obsidian again: Enter picks the newest vault and saves" "0|Saved in ~/.zshenv: t now uses $home/Vaults/Notes/Todo/Todo.md" "$rc|${${(f)out}[-1]##*\] }"
check "init --obsidian again: the earlier line is replaced, the rest kept" "export EDITOR=vi

$MARK
export TODO_FILE=\"\${TODO_FILE:-\$HOME/Vaults/Notes/Todo/Todo.md}\"" "$(<"$home/.zshenv")"
ttyinit 1 init --obsidian
check "init --obsidian: an existing list is used as it is" \
  "$home/Vaults/Notes/Todo/Todo.md already exists; using it as it is|~/.zshenv already points t at this list" \
  "${${(@f)out}[-2]##*\] }|${${(@f)out}[-1]}"

vhome Solo
ttyinit n init --obsidian
check "init --obsidian, one vault, not saved: shows the line" "0|Obsidian vault: $home/Vaults/Solo
Created $home/Vaults/Solo/Todo/Todo.md
Use this list from now on, also from AI agents? This adds TODO_FILE to ~/.zshenv. [Y/n] Not saved. To use the list, add this to ~/.zshenv:
  export TODO_FILE=\"\${TODO_FILE:-\$HOME/Vaults/Solo/Todo/Todo.md}\"" "$rc|$out"
[[ ! -e $home/.zshenv ]]; check "init --obsidian, not saved: no ~/.zshenv written" 0 $?

vhome Solo
tinit init --obsidian
check "init --obsidian without a terminal, one vault: asks nothing, shows the line" "0|Obsidian vault: $home/Vaults/Solo
Created $home/Vaults/Solo/Todo/Todo.md
To use it, add this to ~/.zshenv:
  export TODO_FILE=\"\${TODO_FILE:-\$HOME/Vaults/Solo/Todo/Todo.md}\"" "$rc|$out"
vhome Work Notes
tinit init --obsidian
check "init --obsidian without a terminal, several vaults: fails, listing them" "1|t: found 2 Obsidian vaults; run t init --obsidian in a terminal to choose one, or t init <path>:
  $home/Vaults/Notes
  $home/Vaults/Work" "$rc|$out"

vhome Solo; print -r -- 'export TODO_FILE="$HOME/elsewhere.md"' >"$home/.zshenv"
before=$(<"$home/.zshenv")
ttyinit "" init --obsidian
check "init --obsidian: a TODO_FILE set elsewhere in ~/.zshenv is left alone" "0|~/.zshenv already sets TODO_FILE, so it's left alone:
  1: export TODO_FILE=\"\$HOME/elsewhere.md\"
To use this list, change that to:
  export TODO_FILE=\"\${TODO_FILE:-\$HOME/Vaults/Solo/Todo/Todo.md}\"" "$rc|${(F)${(@f)out}[3,-1]}"
check "init --obsidian: that ~/.zshenv is unchanged" "$before" "$(<"$home/.zshenv")"

vhome 'My "odd" $vault'
ttyinit y init --obsidian
check "init --obsidian: odd vault names are quoted for ~/.zshenv" "$home/Vaults/My \"odd\" \$vault/Todo/Todo.md" \
  "$(HOME=$home zsh -fc 'unset TODO_FILE; source "$HOME/.zshenv"; print -r -- $TODO_FILE')"

vhome Work Notes
ttyinit 7 init --obsidian; check "init --obsidian: a choice out of range fails" "1|t: no choice '7'; nothing done" "$rc|${${(f)out}[-1]##*\] }"
ttyinit 0 init --obsidian; check "init --obsidian: 0 isn't a choice" 1 "$rc"
ttyinit $'\C-d' init --obsidian; check "init --obsidian: Ctrl-D, nothing done" "1|t: no answer; nothing done" "$rc|${${(f)out}[-1]}"
[[ ! -e $home/Vaults/Work/Todo && ! -e $home/Vaults/Notes/Todo ]]; check "init --obsidian: failed choices create nothing" 0 $?
tinit init --obsidian extra; check "init --obsidian with an extra argument fails" "1|t: usage: t init [<path> | --obsidian]" "$rc|$out"
tinit init --bogus;          check "init with an unknown option fails" "1|t: usage: t init [<path> | --obsidian]" "$rc|$out"

home=$(mktemp -d); dirs+=$home
tinit init --obsidian
check "init --obsidian without vaults fails" \
  "1|t: found no Obsidian vaults (Obsidian lists a vault once you've opened it); use t init <path> instead" "$rc|$out"

# t init <path>: never asks; shows the ~/.zshenv line. ${home:A}: on macOS, /var is a symlink.
home=$(mktemp -d); dirs+=$home; home=${home:A}
out=$(unset TODO_FILE; cd "$home"; HOME=$home ZDOTDIR=$home XDG_CONFIG_HOME=$home/.config APPDATA=$home/AppData "$T_BIN" init "lists/Todo.md" </dev/null 2>&1); rc=$?
check "init <path>: relative to the current directory, shows the ~/.zshenv line" "0|Created $home/lists/Todo.md
To use it, add this to ~/.zshenv:
  export TODO_FILE=\"\${TODO_FILE:-\$HOME/lists/Todo.md}\"" "$rc|$out"
tinit init "$home/lists/Todo.md"; check "init <path> refuses an existing list" "1|t: $home/lists/Todo.md already exists; not changed" "$rc|$out"
tinit init a b; check "init with two paths fails" "1|t: usage: t init [<path> | --obsidian]" "$rc|$out"

# Plain t init: offers the vaults only in a terminal, and only when TODO_FILE isn't set.
vhome Solo
tinit init
check "plain init without a terminal: the default place, no questions" "0|Created $home/todo/Todo.md" "$rc|$out"
vhome Solo
ttyinit $'y\ny' init
check "plain init in a terminal offers the vault" "0|Keep the list in your Obsidian vault, in $home/Vaults/Solo/Todo? [Y/n] Created $home/Vaults/Solo/Todo/Todo.md" "$rc|${(F)${(@f)out}[1]}"
[[ -f $home/Vaults/Solo/Todo/Todo.md && ! -e $home/todo && -f $home/.zshenv ]]; check "plain init in a terminal: list made in the vault, and saved" 0 $?
vhome Solo
ttyinit n init
check "plain init in a terminal, vault declined: the default place" "0|Keep the list in your Obsidian vault, in $home/Vaults/Solo/Todo? [Y/n] Created $home/todo/Todo.md" "$rc|$out"
vhome Work Notes
ttyinit 0 init
check "plain init in a terminal, several vaults: 0 keeps the default place" "Created $home/todo/Todo.md" "${${(@f)out}[-1]##*\] }"

# --- several lists: t use alone in a terminal, and old archives when another list is named ---
fresh "$EMPTY"; printf '## Today\n- [ ] Ship it\n\n## Later\n\n## Done\n' >"$dir/Work.md"
zpty -b TU 'stty -echo; PATH="$fakebin:$PATH" PICK=Work "$T_BIN" use; print -r -- "<rc=$?>"'
buf=; for i in {1..400}; do while zpty -r -t TU chunk 2>/dev/null; do buf+=$chunk; done; [[ $buf == *'<rc='<->'>'* ]] && break; zselect -t 5; done
zpty -d TU; buf=${buf//$'\r'/}
check "use alone in a terminal picks a list with fzf" "Now using Work (1 open)|<rc=0>" "${(j:|:)${(f)buf}}"
fresh "$EMPTY"; printf '## Today\n\n## Later\n\n## Done\n' >"$dir/Work.md"
printf '# Archive 2020\n\n- [x] 2020-05-01 Ancient\n' >"$dir/Archive 2020.md"
run @Work list
[[ -f "$dir/Archive/Todo 2020.md" && ! -e "$dir/Archive 2020.md" ]]
check "old archives move to the default list's, also when another list is named" 0 $?
run list --done --all; check "moved archives still show with list --done --all" "2020-05-01  Ancient" "$out"

# names are text, never patterns
fresh "$EMPTY"; run lists add Work; run lists add 'W*'
run @'w*' add star;      check "a name with * matches only that list" "Added 1 (Today): star" "$out"
run @Work list;          check "the other list is untouched" "" "$out"
# rename keeps the list current by t use's choice, even while TODO_LIST says otherwise
run use Work; out=$(TODO_LIST=Todo "$T_BIN" lists rename Work Job 2>&1)
check "rename with TODO_LIST set keeps .current right" "Job" "$(<"$dir/.current")"
# another list doesn't need the default list's lock (only to move old archives)
hold 1500
out=$("$T_BIN" @Job add quick 2>&1); rc=$?
check "a named list works while the default list is locked" "0|Added 1 (Today): quick" "$rc|$out"
kill -9 $holder; wait $holder 2>/dev/null

# --- t move: an item to another list ---
fresh $'## Today\n- [ ] Fix the build\n- [ ] Buy milk\n\n## Later\n- [ ] Paint the fence\n\n## Done\n'
run lists add Work
run move build @work;    check "move by text keeps the section" "0|Moved to Work (Today) as 1: Fix the build" "$rc|$out"
run mv 2 @Work;          check "mv by number, from Later to Later" "0|Moved to Work (Later) as 2: Paint the fence" "$rc|$out"
check "move: gone from the source list" $'## Today\n- [ ] Buy milk\n\n## Later\n\n## Done' "$(file)"
check "move: in the target list" $'## Today\n- [ ] Fix the build\n\n## Later\n- [ ] Paint the fence\n\n## Done' "$(<"$dir/Work.md")"
run @Work move fence @Todo; check "move from a named list" "Moved to Todo (Later) as 2: Paint the fence" "$out"
run @Work undo; run undo
check "move: undo in each list puts it back" $'## Today\n- [ ] Buy milk\n\n## Later\n\n## Done' "$(file)"
check "move: undo in the target list takes it out" $'## Today\n- [ ] Fix the build\n\n## Later\n- [ ] Paint the fence\n\n## Done' "$(<"$dir/Work.md")"
before=$(file)
run move milk @Todo;     check "move to the same list fails" "1|t: the item is already in Todo" "$rc|$out"
run move milk;           check "move without a list fails" "1|t: usage: t move <item> @<list>" "$rc|$out"
run move milk @Nope;     check "move to an unknown list fails" "1|t: no list called 'Nope'; t lists shows them" "$rc|$out"
run move nothing @Work;  check "move of an unknown item fails" "1|t: no open item matches 'nothing'" "$rc|$out"
printf '## Later\n\n## Done\n' >"$dir/Later only.md"
run move milk @"Later only"; check "move to a list without that section fails" 1 "$rc"
check "failed moves change nothing" "$before" "$(file)"
check "failed move leaves the target alone" $'## Later\n\n## Done' "$(<"$dir/Later only.md")"
run complete move;       check "complete move: the items" $'1\t  1  Today  Buy milk' "$out"
run complete move 1;     check "complete move <item>: the lists, as @<name>" "@'Later only' |@Todo |@Work " "${(j:|:)${(@)${(f)out}%%$'\t'*}}"
# both lists are locked: a move waits for the target list's lock
hold_file() { local saved=$TODO_FILE; TODO_FILE=$1; hold $2; TODO_FILE=$saved; }
hold_file "$dir/Work.md" 80
start=$EPOCHREALTIME; out=$("$T_BIN" move milk @Work 2>&1); rc=$?
check "move waits for the target list's lock" "0|Moved to Work (Today) as 2: Buy milk|waited" \
  "$rc|$out|$( (( EPOCHREALTIME - start > 0.5 )) && print waited || print 'did not wait')"
wait $holder

print "$pass passed, $fail failed"
(( fail == 0 ))
