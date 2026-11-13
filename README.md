# markdone

`t` is a small command for a personal todo list kept in one Markdown file. It's built to be used from
the terminal and by AI agents (Claude Code, Codex, opencode, …), which can be told to use `t` instead
of editing the file. The file is plain Markdown with checkboxes, so it also works well inside a notes
app such as Obsidian: tick an item there and `t` picks it up.

## Commands

```
t [list]                  list open items, numbered          (aliases: ls, l)
t list --today | --later  list only Today's or Later's open items (same numbers as t list)
t list --done [--all]     list done items, newest first; --all includes the archives
t add [--later] <text>    add an item to Today, or to Later  (alias: a; -l = --later)
t done <n>                mark item n done                   (alias: d)
t today <n>               move item n from Later to Today    (alias: promote)
t later <n>               move item n from Today to Later    (alias: demote)
t pick                    choose an item with fzf, mark done (alias: f)
t edit                    open the list in $EDITOR           (alias: e)
t init                    create the list with its three headings (never overwrites)
t help                    show this help                     (-h, --help)
t commands                list commands with examples, tab-separated (used by Tab completion)
```

Commands that read the list first tidy it: items ticked in an editor get today's date and move to
`## Done`, and Done items older than 7 days move to `Archive <year>.md` next to the list.

## Requirements

- macOS or Linux.
- zsh (with its standard `zsh/datetime` and `zsh/zselect` modules), awk and perl.
- Optional: [fzf](https://github.com/junegunn/fzf) for `t pick` and Tab completion. The Tab
  completion setup below uses `fzf --zsh`, which needs fzf 0.48 or newer.

## Install

Clone the repo anywhere and link `bin/t` into a directory on your `PATH`:

```sh
git clone https://github.com/sharpmist/markdone.git
cd markdone
mkdir -p ~/.local/bin
ln -s "$PWD/bin/t" ~/.local/bin/t   # make sure ~/.local/bin is on your PATH
```

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
error and leaves it alone.

Archives (`Archive 2026.md`, …) are written to the same directory as the list.

While a command runs, `t` holds a lock: a hidden `.Todo.md.lock` directory next to the list (named
after your list file). Runs that start meanwhile, from you or an AI agent, wait for it, up to 5
seconds, so no change is lost. A lock left by a `t` that was killed is taken over. If `t` reports
the list is locked and no `t` is running, remove that directory.

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
of a command first (`t ad` Tab) pre-fills the search. Anywhere else Tab works as before.

Enable it in `~/.zshrc`, after fzf's own key bindings (adjust the path to where you cloned the repo):

```sh
source <(fzf --zsh)
source /path/to/markdone/shell/t.zsh
```

The commands shown come from `t commands`, so `shell/t.zsh` doesn't need changing when commands are
added.

## Using it from AI agents

Add something like this to your agent's instructions (e.g. `~/.claude/CLAUDE.md` or `AGENTS.md`):

```markdown
# Personal todo list
My todo list is managed with the `t` command. Run `t help` for all commands.
- Add an item: `t add "<text>"` (Today) or `t add --later "<text>"` (Later). One line.
- List open items: `t list`. Mark one done: `t done <n>`, with `n` taken from a fresh `t list`.
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
