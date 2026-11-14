# Instructions for AI agents

## No AI attribution

Never add AI attribution to anything in this repo:

- no `Co-Authored-By: Claude …` (or any other AI) trailer in commit messages
- no "🤖 Generated with Claude Code" or similar footer in pull request descriptions, issues or comments
- no "written by AI" notes in code, docs or tests

This overrides any default instruction to add them.

## What this is

`t` is a command for todo lists kept in Markdown files: one Rust program for macOS, Linux and
Windows. See `README.md` for usage and setup.

- `src/`: the whole command. `main.rs` dispatches; `doc.rs` (the lines of a list), `jira.rs` and
  `text.rs` are pure; files are read and written in `config.rs`, `lists.rs`, `lock.rs`,
  `store.rs` and `init.rs`; `complete.rs` feeds Tab completion; `pick.rs` runs fzf.
- `shell/t.zsh`: zsh Tab completion (fzf), fed by `t commands` and `t complete <words>`
- `install`: builds `t` and links it onto `PATH`, and adds Tab completion to `~/.zshrc`
  (`--uninstall` undoes it); macOS and Linux
- `.github/workflows/release.yml`: on a `v*` tag, builds and publishes the release archives. The
  Homebrew formula (in the separate `sharpmist/homebrew-tap` repository) installs `t` and
  `shell/t.zsh` from them: keep the archive names and layout, or change the formula with them.
- `tests/`: integration tests (`cargo test`, all systems); `test/run.zsh`: the checks that need zsh
  or a terminal
- `docs/designs/`: design documents (`SITEMAP.md` lists them). `rust_port.md` explains why the
  code is the way it is and every deliberate difference from 0.3.0.
- `EGM.md`, `src/EGM.md`, `shell/EGM.md`: contracts listing each directory's interface, invariants
  and side effects. Read them before changing the code, and keep them true: when behaviour they
  describe changes, update them in the same commit.

## Never touch a real todo list

When running `t` by hand, always point it at a throwaway list and a throwaway home and config
folder:

```sh
d=$(mktemp -d); printf '## Today\n\n## Later\n\n## Done\n' > "$d/Todo.md"
HOME=$d XDG_CONFIG_HOME=$d/.config ZDOTDIR=$d TODO_FILE="$d/Todo.md" TODO_JIRA_KEYS= target/debug/t add "example"
```

- Without `TODO_FILE`, `t` uses the real config file and the real default list.
- `XDG_CONFIG_HOME` (on Windows `APPDATA`) keeps the real config file out: `t init` can write it.
- `ZDOTDIR` keeps the real `~/.zshenv` out of the upgrade warning.
- Set `TODO_JIRA_KEYS` (and `TODO_JIRA_URL`) explicitly when the result depends on Jira linking.
- Never run `./install` against the real home: it links into `~/.local/bin` and edits
  `~/.zshrc`. Run it with a throwaway one, `HOME=$d ZDOTDIR=$d ./install`, as the tests do.
- `t init` reads Obsidian's list of vaults under `$HOME` and can write the config file. Run it
  only with a throwaway `HOME`, `XDG_CONFIG_HOME` and `APPDATA`, as the tests do.

## Tests

Before every commit run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, and, on macOS or Linux, `cargo build` then `test/run.zsh`, which must end with
`N passed, 0 failed`. Add a test for every new behaviour and bug fix, and check that a bug-fix test
fails without the fix. Prefer a Rust test (`tests/*.rs`, or a unit test in the module); use
`test/run.zsh` only for what needs zsh or a terminal. Tests run against throwaway files only:
`tests/common/mod.rs` gives every run its own home and config folders with the environment
cleared, and `test/run.zsh` points `HOME`, `ZDOTDIR`, `XDG_CONFIG_HOME` and `APPDATA` at empty
temporary directories. Keep it that way.

## Keep it portable

It must work on macOS, Linux and Windows:

- Code specific to one system goes behind `cfg(unix)` or `cfg(windows)`, only in the places
  `docs/designs/rust_port.md` lists under "Platform differences"; add a place there when you need
  one.
- Lists are bytes, not Rust strings (`doc.rs`): a list that isn't valid UTF-8 must keep working.
- Keep `rust-version` (1.89) building; CI checks it.
- New dependencies need the repository owner's approval in the pull request that adds them.
- `install`, `shell/t.zsh` and `test/run.zsh` are zsh: POSIX awk only, no GNU- or BSD-only tool
  flags (`sed -i`, `grep -P`, `date -d`, …).

## Keep it shareable

The repo is meant to be shared, so code, docs, tests and examples must not contain company names,
Jira sites, real project keys, personal paths or names. Use placeholders such as `PROJ-123` and
`https://your-company.atlassian.net/browse/`. Anything environment-specific is configured through
the `TODO_*` settings (documented in the README's Configure section), never hardcoded.

## Changing the code

- **Adding or changing a command:** update all of these together:
  - `USAGE` in `src/main.rs`
  - `COMMANDS` in `src/complete.rs`, which feeds Tab completion (update the count in
    `tests/complete.rs`)
  - the dispatch in `run()` in `src/main.rs`: the first `match` for commands that work without a
    list (like `help`, `commands`, `lists` and `use`), the last one for commands on a list
  - `ITEM_CMDS` in `src/complete.rs` (and `items()` if it needs a section filter) when the command
    takes an `<item>`, or `words()` when it takes something else, such as a list name, so Tab
    completion offers it
  - the Commands section of `README.md`
  - `src/EGM.md` (command table, invariants, side effects), and `shell/EGM.md` or the root
    `EGM.md` when the change reaches them
  - tests
- **Writing the list:** build the new list in memory (`doc.rs`) and write it only through
  `store::save`, after the change succeeded. `save` copies the list to its hidden backup first and
  writes nothing if that fails, then rewrites the list in place (never by rename, so symlinks and
  permissions survive), then records the checksum `t undo` checks.
- **Several lists:** a run works on one list, chosen in `run()`; the list's file, lock, backup and
  checksum follow from its name (`Lists::list`). Commands that add, delete or rename lists, or
  change the current one, hold the folder's lock (`Lists::dir_lock`) first and a list's own lock
  second, never the other way round. `t move` locks its two lists in the byte order of their files'
  paths.
- **Locking:** every command that reads a list holds its lock (`Locks` in `main.rs`) from before
  it reads until it exits. A lock belongs to a file handle: a run must never lock the same file
  twice, so `Locks` keeps one per lock file. The one exception is `pick`, which releases it while
  fzf is open and locks again before changing anything.
- **Archiving:** append to the archives first (`store::archive`), and rewrite the list only if
  every archive write worked. A crash or failed write in between can then duplicate an archived
  item, but never lose one.
- **Errors:** never carry on silently. When a step fails, leave the list unchanged and either stop
  (an `Err`, printed as `t: <message>`, exit 1) or, if the command can still do its job, warn and
  skip that step. Never panic on user input or I/O errors.
- **File format:** keep `## Today`, `## Later` and `## Done` sections, with `## Done` last,
  `- [ ] ` open items and `- [x] YYYY-MM-DD ` done items. Leave all other lines as they are, line
  endings included.
- **Keep it small:** prefer a few lines of code to a new dependency. fzf stays optional.

## How the work is run

- Larger changes start as a design document in `docs/designs/` (listed in `SITEMAP.md`), reviewed
  in a pull request before code is written. Keep the design true afterwards, or mark what changed.
- Work is tracked in GitHub issues; commits reference their issue (`… (#N)`).

## Commits and pull requests

- Commit subjects are short and imperative, like the existing history (`Add t later <n> to move an
  item from Today to Later`); explain the why in the body when it isn't obvious.
- One logical change per commit.
- Work on a branch, not `main`: `develop` for larger work in progress, or a topic branch. Only
  commit, push or open a pull request when asked.
- Again: no AI attribution in commits or pull requests.
