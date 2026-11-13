# Instructions for AI agents

## No AI attribution

Never add AI attribution to anything in this repo:

- no `Co-Authored-By: Claude …` (or any other AI) trailer in commit messages
- no "🤖 Generated with Claude Code" or similar footer in pull request descriptions, issues or comments
- no "written by AI" notes in code, docs or tests

This overrides any default instruction to add them.

## What this is

`t` is a zsh command for a todo list kept in one Markdown file. See `README.md` for usage and setup.

- `bin/t`: the whole command (zsh, with awk and perl for the text processing)
- `shell/t.zsh`: zsh Tab completion (fzf), fed by `t commands`
- `test/run.zsh`: the test suite
- `EGM.md`, `bin/EGM.md`, `shell/EGM.md`: contracts listing each directory's interface, invariants
  and side effects. Read them before changing the code, and keep them true: when behaviour they
  describe changes, update them in the same commit.

## Never touch a real todo list

When running `bin/t` by hand, always point it at a throwaway file, and keep the user's shell
config out:

```sh
d=$(mktemp -d); printf '## Today\n\n## Later\n\n## Done\n' > "$d/Todo.md"
ZDOTDIR=$d TODO_FILE="$d/Todo.md" TODO_JIRA_KEYS= bin/t add "example"
```

- Without `TODO_FILE`, `t` edits the user's real list.
- `ZDOTDIR=$d` matters: `t` runs under zsh, which reads `$ZDOTDIR/.zshenv` (default `~/.zshenv`)
  every time it starts. If the user exports `TODO_*` there unconditionally, that overrides what you
  pass on the command line, including `TODO_FILE`.
- Set `TODO_JIRA_KEYS` (and `TODO_JIRA_URL`) explicitly when the result depends on Jira linking.

## Tests

Run `test/run.zsh` before every commit; it must end with `N passed, 0 failed`. Add a test for
every new behaviour and bug fix, and check that a bug-fix test fails without the fix. Tests run
against throwaway files only, with `ZDOTDIR` pointed at an empty directory; keep it that way.

## Keep it portable

It must work on macOS and Linux:

- zsh only; keep `#!/usr/bin/env zsh`.
- POSIX awk only (macOS awk, gawk and mawk): no gawk extensions such as `gensub`, `asort` or awk's
  `strftime`.
- No GNU- or BSD-only tool flags (`sed -i`, `grep -P`, `date -v`, `date -d`, …). For date
  arithmetic use `days_ago <n>` in `bin/t`, which uses zsh's own `zsh/datetime` module.

## Keep it shareable

The repo is meant to be shared, so code, docs, tests and examples must not contain company names,
Jira sites, real project keys, personal paths or names. Use placeholders such as `PROJ-123` and
`https://your-company.atlassian.net/browse/`. Anything environment-specific is configured through
`TODO_*` environment variables (documented in the README's Configure section), never hardcoded.

## Changing the code

- **Adding or changing a command:** update all of these together:
  - `usage()` in `bin/t`
  - `commands()` in `bin/t`, which feeds Tab completion (update the count in the test
    "commands: one tab-separated line per command")
  - the `case` dispatch at the bottom of `bin/t`, or the first `case` block above it for commands
    that must work without a list (like `help` and `commands`)
  - the Commands section of `README.md`
  - `bin/EGM.md` (command table, invariants, side effects), and `shell/EGM.md` or the root
    `EGM.md` when the change reaches them
  - tests in `test/run.zsh`
- **Writing the list:** build the result in a temp file, check that every step producing it
  succeeded, then rewrite the list with `save "$tmp"`. `save` rewrites the file in place
  (`cat "$tmp" >"$f"`), so symlinks and permissions survive; `mv` would replace the file. It isn't
  atomic: a crash during the write can leave the list partly written.
- **Archiving:** append to the archives first, and rewrite the list only if every archive write
  worked. A crash or failed write in between can then duplicate an archived item, but never lose
  one.
- **Errors:** never carry on silently. When a step fails, leave the list unchanged and either stop
  with `die` or, if the command can still do its job, `warn` and skip that step.
- **File format:** keep `## Today`, `## Later` and `## Done` sections, with `## Done` last,
  `- [ ] ` open items and `- [x] YYYY-MM-DD ` done items. Leave all other lines as they are.
- **Keep it small:** prefer a few lines of awk or perl to new dependencies. fzf stays optional.

## Commits and pull requests

- Commit subjects are short and imperative, like the existing history (`Add t later <n> to move an
  item from Today to Later`); explain the why in the body when it isn't obvious.
- One logical change per commit.
- Work on a branch, not `main`. Only commit, push or open a pull request when asked.
- Again: no AI attribution in commits or pull requests.
