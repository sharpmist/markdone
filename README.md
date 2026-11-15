# markdone

A personal todo list kept in plain Markdown files, with a small command, `t`, that you and your AI
agents can both use safely.

- **Plain Markdown.** `## Today`, `## Later` and `## Done` sections with checkboxes. Open a list in
  Obsidian or any editor; tick an item there and `t` picks it up.
- **One list or several.** Keep work and home apart: `t @work add …`, or `t use work` to switch.
- **Made for AI agents too.** Agents (Claude Code, Codex, opencode, …) name items by a part of
  their text instead of numbers that shift, can read `t list --json`, and never need to edit the
  file.
- **Safe to share.** Runs from you and your agents take turns, every change is backed up first,
  and `t undo` reverses the last one.
- **Small.** One program, for macOS, Linux and Windows, with nothing else to install. fzf is
  optional.

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
t move <item> @<list>     move an item to another list, same section (alias: mv)
t pick                    choose an item with fzf, mark done (alias: f)
t edit                    open the list in $EDITOR           (alias: e)
t undo                    undo the last change; run it again to redo
t init [<path>]           create the list with its three headings (never overwrites)
t init --obsidian         create the list in an Obsidian vault you pick, in its Todo folder

t lists [--json]          show the lists, with open items; * marks the current one
t lists add <name>        create another list, next to the default one
t lists rm [--force] <name>   delete a list (with open items only with --force)
t lists rename <old> <new>    rename a list and its archives
t use [<list>]            switch the current list (alone: pick one with fzf)
t @<list> <command>       run one command on another list, e.g. t @work add "Fix the build"

t help                    show this help                     (-h, --help)
t version                 show the version                   (--version)
t commands                list commands with examples, tab-separated (used by Tab completion)
t complete <words>        list what Tab offers after t <words>, tab-separated (used by Tab completion)
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
{"list":"Todo","n":1,"section":"Today","text":"Retest logout fix [PROJ-123](https://your-company.atlassian.net/browse/PROJ-123)","jira":["PROJ-123"]}
]
```

`list` is the list's name; `text` is the item as stored in the file (Markdown); `jira` lists the
Jira keys in it (empty when Jira linking is off).

Commands that read a list first tidy it: items ticked in an editor get today's date and move to
`## Done`, and Done items older than 7 days move to `Archive/<list> <year>.md` in the lists folder.

### Several lists

All your lists live in one folder: the one that holds `TODO_FILE`, your default list. Every `.md`
file directly in it is a list, named after the file: `Todo.md` is `Todo`, `Work.md` is `Work`. In
Obsidian that's the vault's `Todo` folder.

```console
$ t lists add Work
Created the list Work; use it with t @Work … or t use Work
$ t @work add "Fix the build"
Added 1 (Today): Fix the build
$ t use work
Now using Work (1 open)
$ t lists
  Todo   3 open
* Work   1 open
```

- **`t @<list> <command>`** runs one command on another list, without switching: `t @work done
  build`, `t @home list`. Names match ignoring case.
- **`t use <list>`** switches the current list, the one every command without `@<list>` uses, in
  all your terminals (it's saved in a hidden `.current` file in the folder). `t use` alone picks one
  with fzf; `t use Todo` (your default list) goes back.
- **`t move <item> @<list>`** moves an item to another list, into the same section:
  `t move build @work`, `t @work move 2 @Todo`.
- **`TODO_LIST`** names the list for one terminal or one agent, overriding `t use`:
  `export TODO_LIST=work`.

So a command uses `@<list>` if given, else `TODO_LIST`, else the list chosen with `t use`, else
the default list. If the current list is deleted outside `t`, it warns and uses the default list.

Each list has its own undo, lock and archives (`Archive/Work 2026.md`). `t move` locks both lists
and changes both, adding the item to the target before removing it from the source, so undoing a
move takes `t undo` in each list (`t @work undo` and `t undo`). `t lists rm` deletes a
list's file but keeps its archives; it refuses the default list, the current list and, without
`--force`, a list with open items. The default list can't be renamed or deleted, since
`TODO_FILE` names it.

Archives from before there were several lists (`Archive 2026.md` next to the list) are moved to
`Archive/<default list> 2026.md` automatically.

## Requirements

- macOS, Linux or Windows.
- To build it: [Rust](https://rustup.rs) 1.89 or newer (or take a release binary, see below).
- Optional: [fzf](https://github.com/junegunn/fzf) for `t pick` and `t use` alone.
- Optional, macOS and Linux: zsh, for `./install` and [Tab completion](#tab-completion-zsh). The Tab
  completion setup below uses `fzf --zsh`, which needs fzf 0.48 or newer.

## Install

On macOS or Linux with [Homebrew](https://brew.sh):

```sh
brew install sharpmist/tap/markdone
```

This installs the release binary, so no Rust is needed. It doesn't edit `~/.zshrc`; to turn on
[Tab completion](#tab-completion-zsh), add the `source` line it prints. `brew upgrade markdone`
updates `t`, and `brew uninstall markdone` removes it. Neither touches your lists.

Or clone the repo anywhere and run `./install`:

```sh
git clone https://github.com/sharpmist/markdone.git
cd markdone
./install
```

`./install` does three things, and says what it did:

- It builds `t` (`cargo build --release`; `--no-build` skips this and links what's built).
- It links the built `t` into `~/.local/bin` (`--bin <dir>` picks another directory), and warns if
  that directory isn't on your `PATH`. After a `git pull`, run `./install` again to rebuild.
- If fzf is installed, it turns on [Tab completion](#tab-completion-zsh) by adding a `source` line
  to the end of `~/.zshrc` (`--no-zshrc` skips this).

It also warns when your `~/.zshrc` defines its own `t`, such as `alias t="tmux"`, which would hide
this one. It never replaces another `t` (a link left by markdone 0.3 is replaced, since it's this
one), and running it again only does what isn't done yet. `./install --uninstall` removes the link
and the `~/.zshrc` line. It doesn't touch your lists.

Anywhere, including Windows, you can instead build and install it with cargo, which puts `t` in
`~/.cargo/bin` (on Windows `%USERPROFILE%\.cargo\bin`):

```sh
cargo install --path .
```

Or download a binary for your system from the
[releases](https://github.com/sharpmist/markdone/releases) and put `t` (`t.exe` on Windows)
somewhere on your `PATH`. The binaries aren't signed: on macOS, allow one with
`xattr -d com.apple.quarantine t`; on Windows, choose "More info" and "Run anyway" the first time.

Then open a new terminal and create your list with `t init`, or with `t init --obsidian` to keep it
in an Obsidian vault (see [Keeping the list in Obsidian](#keeping-the-list-in-obsidian)).

### Configure

`t` is configured with these settings:

| Setting          | Purpose                                                      | Default          |
| ---------------- | ------------------------------------------------------------ | ---------------- |
| `TODO_FILE`      | The default list; its folder holds all your lists            | `~/todo/Todo.md` |
| `TODO_LIST`      | The list to use, by name, instead of the current one         | unset            |
| `TODO_JIRA_KEYS` | Jira project keys to link, space-separated; empty = off      | unset (no links) |
| `TODO_JIRA_URL`  | Jira browse URL the keys are appended to; must end with `/`  | unset            |

Put them in the config file, so they apply however `t` is started (from any shell, scripts or AI
agents):

- macOS and Linux: `~/.config/markdone/config` (or `$XDG_CONFIG_HOME/markdone/config`)
- Windows: `%APPDATA%\markdone\config`

```sh
# ~/.config/markdone/config
TODO_FILE=~/notes/Todo.md
TODO_JIRA_KEYS=PROJ OPS WEB
TODO_JIRA_URL=https://your-company.atlassian.net/browse/
```

One `NAME=value` per line; lines starting with `#` are comments. Values are taken as they are, with
no quotes and no `$` expansion; `~/` at the start of `TODO_FILE` means your home directory.
`TODO_LIST` can't be set there: it's for one terminal or one agent, so set it in the environment.

An environment variable that is set wins over the file, even when empty: `TODO_FILE=/tmp/test.md
t list` tries a list elsewhere, and `TODO_JIRA_KEYS= t …` turns linking off for one run. An empty
`TODO_FILE` means the default.

`t` doesn't create the list on its own. Create it once:

```sh
t init
```

This writes the three section headings (`## Today`, `## Later`, `## Done`) to the default list,
creating its directory if needed. It never overwrites: if the file already exists, `t init` stops
with an error and leaves it alone. `t init <path>` creates the list at `<path>` instead and shows
the config line that makes `t` use it.

### Keeping the list in Obsidian

`t init` can put the list in an [Obsidian](https://obsidian.md) vault, in a `Todo` folder, so you can
read and tick it in Obsidian too:

```sh
t init --obsidian
```

It reads the vaults Obsidian knows about (any vault you've opened in it) and asks which one to use.
With a single vault it just names it. It then creates `<vault>/Todo/Todo.md` and, if you agree,
writes it to the config file so that `t` and your AI agents use that list from then on:

```sh
# ~/.config/markdone/config
TODO_FILE=~/Documents/My Vault/Todo/Todo.md
```

Running it again replaces that line and keeps the rest of the file. If the vault already has a
list (say, synced from another computer), `t init --obsidian` uses it as it is. A plain `t init` in
a terminal offers your vaults too, unless `TODO_FILE` is already set.

`t init` only asks questions in a terminal. Run elsewhere (by a script or an AI agent), it asks
nothing: it uses the only vault, stops with a list if there are several, and shows the config line
instead of writing it.

In the vault, the archives (`Archive/Todo 2026.md`, …) are ordinary notes in the `Todo` folder's
`Archive` subfolder. The lock, backup and checksum files start with a dot, so Obsidian doesn't
show them.

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

`t edit` opens the list in `$EDITOR`, or `nvim` (`notepad` on Windows) if `$EDITOR` isn't set.
`$EDITOR` may include arguments, e.g. `EDITOR="code -w"`, or be the full path of an editor, spaces
and all.

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
(notes, blank lines) between items are left as they are. Windows line endings (CRLF) work too: the
lines `t` writes take the ending of the file's first line, and the others keep theirs.

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
part of the item first (`t done fix` Tab) pre-fills the search.

With several lists, `t @` Tab picks a list, `t @work ` Tab a command and `t @work done ` Tab one of
that list's items. `t use `, `t lists rm `, `t lists rename ` and `t move <item> ` Tab pick a
list, and `t lists `
Tab one of its subcommands. Anywhere else Tab works as before.

`./install` turns it on. To do it by hand instead, add this to `~/.zshrc`, after fzf's own key
bindings (adjust the path to where you cloned the repo):

```sh
source <(fzf --zsh)
source /path/to/markdone/shell/t.zsh
```

With Homebrew, the file is at `$(brew --prefix)/opt/markdone/share/markdone/t.zsh`.

The commands shown come from `t commands`, and everything else from `t complete <words>`, so
`shell/t.zsh` doesn't need changing when commands are added.

## Windows

`t` works the same in PowerShell, cmd and Windows Terminal. A few things differ:

- The config file is `%APPDATA%\markdone\config`, and `t edit` opens Notepad unless `EDITOR` is set.
  An `EDITOR` like `code -w` works (VS Code's `code` is a `.cmd` file, found through `PATHEXT`).
- `t` writes UTF-8. For non-ASCII item text to show correctly in PowerShell, set
  `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8` (for example in your profile).
- Windows PowerShell 5.1 drops `"` inside arguments it passes to programs; use PowerShell 7, or
  avoid double quotes inside item text there.
- Tab completion and `./install` need zsh, so they're for macOS and Linux.

## Upgrading from 0.3

markdone 0.3 was a zsh script and read its settings from `~/.zshenv`. 0.4 is a program and doesn't
read `~/.zshenv`:

1. Run `./install` again: it builds `t` and replaces the old link. (Or remove that link with
   `./install --uninstall` and install with Homebrew.)
2. Move your settings (`TODO_FILE`, `TODO_JIRA_KEYS`, `TODO_JIRA_URL`) from `~/.zshenv` to the
   [config file](#configure). Until you do, `t` started outside zsh warns and shows the line to
   add; in zsh they still reach `t` from the environment.
3. Your lists, archives, backups and undo history work as they are.

Don't use 0.3 and 0.4 on the same list at the same time (say, an old copy on another computer
syncing the same folder): on Linux their locks don't see each other.

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
- I may have several lists (`t lists` shows them). When I name one, use `t @<list> …`, e.g.
  `t @work add "<text>"`; otherwise leave the current one as it is (don't run `t use`). Move an
  item to another list with `t move "<text>" @<list>`.
- Only touch the list when I ask.
```

## Test

```sh
cargo test
test/run.zsh
```

`cargo test` runs the unit tests and the integration tests in `tests/`, on any system. Then, on
macOS or Linux, `test/run.zsh` runs the checks that need zsh or a terminal (Tab completion, `t pick`,
`t edit`, `t init`'s questions, `./install`) against the built `target/debug/t`; it needs `python3`
and zsh's standard `zsh/zpty` and `zsh/zselect` modules.

Both run `t` with a temporary home folder and config folder, so your real lists, settings and
Obsidian vaults are never touched.

## License

[MIT](LICENSE)
