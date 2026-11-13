# markdone

A personal todo list kept in one plain Markdown file, with a small command, `t`, that you and your
AI agents can both use safely.

- **One Markdown file.** `## Today`, `## Later` and `## Done` sections with checkboxes. Open it in
  Obsidian or any editor; tick an item there and `t` picks it up.
- **Made for AI agents too.** Agents (Claude Code, Codex, opencode, …) name items by a part of
  their text instead of numbers that shift, can read `t list --json`, and never need to edit the
  file.
- **Safe to share.** Runs from you and your agents take turns, every change is backed up first,
  and `t undo` reverses the last one.
- **Small.** One zsh script using awk and perl, for macOS and Linux. fzf is optional.

## A quick look

```console
$ t add "Fix the flaky login test"
Added 1 (Today): Fix the flaky login test
$ t add "Fix a typo in the README"
Added 2 (Today): Fix a typo in the README
$ t add --later "Clean up old branches"
Added 3 (Later): Clean up old branches
$ t list
  1  Today  Fix the flaky login test
  2  Today  Fix a typo in the README
  3  Later  Clean up old branches
$ t done fix
t: 'fix' matches 2 open items; use more of the text, or the number:
  1  Today  Fix the flaky login test
  2  Today  Fix a typo in the README
$ t done flaky
Done: Fix the flaky login test
$ t later typo
Moved to Later as 2: Fix a typo in the README
$ t undo
Undid the last change; run t undo again to redo it
$ t list
  1  Today  Fix a typo in the README
  2  Later  Clean up old branches
```

The list itself stays plain Markdown:

```markdown
## Today
- [ ] Fix a typo in the README

## Later
- [ ] Clean up old branches

## Done
- [x] 2026-11-13 Fix the flaky login test
```

## Commands

```
t [list]                  list open items, numbered          (aliases: ls, l)
t list --today | --later  list only Today's or Later's open items (same numbers as t list)
t list --done [--all]     list done items, newest first; --all includes the archives
t list --json             open items as JSON (also with --today or --later)
t add [--later] <text>    add an item to Today, or to Later  (alias: a; -l = --later)
t done <item>             mark an item done                  (alias: d)
t today <item>            move an item from Later to Today   (alias: promote)
t later <item>            move an item from Today to Later   (alias: demote)
t rm <item>               remove an open item                (alias: remove)
t reword <item> <text>    replace an open item's text, keeping its place
t pick                    choose an item with fzf, mark done (alias: f)
t edit                    open the list in $EDITOR           (alias: e)
t undo                    undo the last change; run it again to redo
t init [<path>]           create the list with its three headings (never overwrites)
t init --obsidian         create the list in an Obsidian vault you pick, in its Todo folder
t help                    show this help                     (-h, --help)
t version                 show the version                   (--version)
t commands                list commands with examples, tab-separated (used by Tab completion)
t complete <command>      list the items <command> takes, tab-separated (used by Tab completion)
```

`<item>` is either a number from `t list` or text that matches exactly one open item: any part of
it, ignoring case, such as `t done "logout fix"` or `t done PROJ-123`. Text is matched against the
item as `t list` shows it in a terminal, so a link counts by its text and its URL never matches. If
the text matches several items, `t` lists them and changes nothing. A reference made only of digits
is always a number.

For `t reword`, the first argument names the item and the rest is the new text, so quote an item
given as text with spaces: `t reword "logout" "Retest logout fix"`.

`t list --json` prints the open items as a JSON array, one object per line, with the same numbers
as `t list`:

```json
[
{"n":1,"section":"Today","text":"Retest logout fix [PROJ-123](https://your-company.atlassian.net/browse/PROJ-123)","jira":["PROJ-123"]}
]
```

`text` is the item as stored in the file (Markdown); `jira` lists the Jira keys in it (empty when
Jira linking is off).

Commands that read the list first tidy it: items ticked in an editor get today's date and move to
`## Done`, and Done items older than 7 days move to `Archive <year>.md` next to the list.

## Requirements

- macOS or Linux.
- zsh 5.8 or newer (with its standard `zsh/datetime` and `zsh/system` modules), awk and perl.
- Optional: [fzf](https://github.com/junegunn/fzf) for `t pick` and Tab completion. The Tab
  completion setup below uses `fzf --zsh`, which needs fzf 0.48 or newer.

## Install

Clone the repo anywhere and run `./install`:

```sh
git clone https://github.com/sharpmist/markdone.git
cd markdone
./install
```

`./install` does two things, and says what it did:

- It links `bin/t` into `~/.local/bin` (`--bin <dir>` picks another directory), and warns if that
  directory isn't on your `PATH`. It's a link, so a `git pull` in the clone updates `t`.
- If fzf is installed, it turns on [Tab completion](#tab-completion-zsh) by adding a `source` line
  to the end of `~/.zshrc` (`--no-zshrc` skips this).

It also warns when your `~/.zshrc` defines its own `t`, such as `alias t="tmux"`, which would hide
this one. It never replaces an existing `t`, and running it again only does what isn't done yet.
`./install --uninstall` removes the link and the `~/.zshrc` line. It doesn't touch your list.

Then open a new terminal and create your list with `t init`, or with `t init --obsidian` to keep it
in an Obsidian vault (see [Keeping the list in Obsidian](#keeping-the-list-in-obsidian)).

### Configure

`t` is configured with environment variables:

| Variable         | Purpose                                                      | Default          |
| ---------------- | ------------------------------------------------------------ | ---------------- |
| `TODO_FILE`      | The list file                                                | `~/todo/Todo.md` |
| `TODO_JIRA_KEYS` | Jira project keys to link, space-separated; empty = off      | unset (no links) |
| `TODO_JIRA_URL`  | Jira browse URL the keys are appended to; must end with `/`  | unset            |

Set them in `~/.zshenv`. `t` runs under zsh, which reads `~/.zshenv` every time it starts, so the
settings apply however `t` is started (from bash, scripts or AI agents too). Write them so they only
fill in values that aren't set yet, as below. A plain `export TODO_FILE=…` there would override
anything set when running `t`, such as `TODO_FILE=/tmp/test.md t list`.

```sh
# ~/.zshenv
export TODO_FILE="${TODO_FILE:-$HOME/notes/Todo.md}"
export TODO_JIRA_KEYS="${TODO_JIRA_KEYS-PROJ OPS WEB}"                              # optional
export TODO_JIRA_URL="${TODO_JIRA_URL-https://your-company.atlassian.net/browse/}"  # optional
```

The Jira lines use `-` instead of `:-` so that `TODO_JIRA_KEYS= t …` still turns linking off for one
run.

`t` doesn't create the list on its own. Open a new terminal (so the settings above are loaded) and
create it once:

```sh
t init
```

This writes the three section headings (`## Today`, `## Later`, `## Done`) to `$TODO_FILE`, creating
its directory if needed. It never overwrites: if the file already exists, `t init` stops with an
error and leaves it alone. `t init <path>` creates the list at `<path>` instead and shows the
`~/.zshenv` line that makes `t` use it.

### Keeping the list in Obsidian

`t init` can put the list in an [Obsidian](https://obsidian.md) vault, in a `Todo` folder, so you can
read and tick it in Obsidian too:

```sh
t init --obsidian
```

It reads the vaults Obsidian knows about (any vault you've opened in it) and asks which one to use.
With a single vault it just names it. It then creates `<vault>/Todo/Todo.md` and, if you agree, adds
a line to `~/.zshenv` so that `t` and your AI agents use that list from then on:

```sh
# markdone: where t keeps the list (set by t init)
export TODO_FILE="${TODO_FILE:-$HOME/Documents/My Vault/Todo/Todo.md}"
```

Running it again replaces that line. If `~/.zshenv` already sets `TODO_FILE` some other way, it's
left alone and `t init` shows what to change. If the vault already has a list (say, synced from
another computer), `t init --obsidian` uses it as it is. A plain `t init` in a terminal offers your
vaults too, unless `TODO_FILE` is already set.

`t init` only asks questions in a terminal. Run elsewhere (by a script or an AI agent), it asks
nothing: it uses the only vault, stops with a list if there are several, and shows the `~/.zshenv`
line instead of adding it.

In the vault, the archives (`Archive 2026.md`, …) are ordinary notes next to the list. The lock,
backup and checksum files start with a dot, so Obsidian doesn't show them.

Archives (`Archive 2026.md`, …) are written to the same directory as the list.

While a command runs, `t` holds a lock on a hidden, empty `.Todo.md.lock` file next to the list
(named after your list file). Runs that start meanwhile, from you or an AI agent, wait for it, up to
10 seconds, so no change is lost. The system releases the lock when `t` ends, even if it's killed,
so it can't get stuck. `t pick` doesn't hold it while fzf is open, so it never keeps others waiting;
if the item you pick was changed meanwhile, it does nothing and says so.

Before every change, `t` copies the list to a hidden `.Todo.md.bak` next to it, so a crash while
writing can't lose it. `t undo` puts that copy back, undoing the last change `t` made; running it
again redoes the change. It refuses when the list was changed since `t` last wrote it (in an editor
or by a sync), so it never throws such a change away: `t` keeps a checksum of what it last wrote
in a hidden `.Todo.md.sum` and compares the list with it. A change that archived done items can't be
undone, since that would archive them twice.

`t edit` opens the list in `$EDITOR`, or `nvim` if `$EDITOR` isn't set. `$EDITOR` may include
arguments, e.g. `EDITOR="code -w"`.

## File format

```markdown
## Today
- [ ] Retest logout fix (PROJ-123)

## Later
- [ ] Clean up old branches

## Done
- [x] 2026-11-11 Merge PR #42
```

You can edit the file by hand or in any Markdown editor; `t` keeps it in this shape. Other lines
(notes, blank lines) between items are left as they are.

Keep `## Done` as the last section. Ticked items are moved into `## Done` only when it comes after
them; otherwise `t` warns and leaves the list untidied.

## Jira links (optional)

When `TODO_JIRA_KEYS` and `TODO_JIRA_URL` are set (see [Configure](#configure)), Jira keys in items
(`PROJ-123`, …) are stored as Markdown links to your Jira site, so they're clickable in Markdown
editors. `t list` shows them as clickable keys in terminals that support OSC 8 hyperlinks (e.g.
WezTerm, iTerm2, kitty) and prints the plain Markdown when the output isn't a terminal (pipes, AI
agents). Keys are linked by `t add` and by the tidy step, so keys typed in an editor get linked too.
Existing links, URLs and `inline code` are left alone.

Only the listed project keys count, so text like `UTF-8` stays as is. Linking is off when
`TODO_JIRA_KEYS` is empty or unset. If the keys aren't valid Jira project keys (uppercase letters,
digits and `_`, separated by spaces) or the URL doesn't start with `http://` or `https://` and end
with `/`, `t` warns and links nothing, so broken links never end up in the list.

## Tab completion (zsh)

Type `t ` and press Tab to pick a command in fzf; each line shows a description and an example.
The choice is put on the command line (for `add`, with the cursor between the quotes). Typing part
of a command first (`t ad` Tab) pre-fills the search.

After a command that takes an item (`t done `, `t rm `, `t reword `, …), Tab picks the item in fzf
instead and puts its number on the command line. `t today ` offers only Later's items and
`t later ` only Today's; for `t reword` the cursor lands between quotes for the new text. Typing
part of the item first (`t done fix` Tab) pre-fills the search. Anywhere else Tab works as before.

`./install` turns it on. To do it by hand instead, add this to `~/.zshrc`, after fzf's own key
bindings (adjust the path to where you cloned the repo):

```sh
source <(fzf --zsh)
source /path/to/markdone/shell/t.zsh
```

The commands shown come from `t commands`, and the items from `t complete <command>`, so
`shell/t.zsh` doesn't need changing when commands are added.

## Using it from AI agents

Add something like this to your agent's instructions (e.g. `~/.claude/CLAUDE.md` or `AGENTS.md`):

```markdown
# Personal todo list
My todo list is managed with the `t` command. Run `t help` for all commands.
- Add an item: `t add "<text>"` (Today) or `t add --later "<text>"` (Later). One line.
- List open items: `t list`, or `t list --json` to parse them.
- Name an item by a unique part of its text, not by number: numbers shift when the list changes.
  If the text matches several items, `t` lists them; use more of the text.
- Mark one done: `t done "<text>"`. Remove one: `t rm "<text>"`. Change one:
  `t reword "<text>" "<new text>"`. Undo your last change: `t undo`.
- Never edit the todo file or its archives directly; `t` keeps the format and archives done items.
- Only touch the list when I ask.
```

## Test

```sh
test/run.zsh
```

Runs `bin/t` against throwaway files, with an example Jira setup. It ignores your `~/.zshenv`, so
your real list and settings are never touched. Besides the requirements above, it needs `python3`
(for the terminal-output checks) and zsh's standard `zsh/zpty` and `zsh/zselect` modules (for the
Tab completion checks).

## License

[MIT](LICENSE)
