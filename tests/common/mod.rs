//! The sandbox every integration test runs `t` in: a temporary directory holding the home
//! directory, the config folders and the lists, and an environment with nothing of the real one
//! but `PATH` (plus what Windows needs to start programs). So no test can reach a real list,
//! config file or Obsidian vault.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const URL: &str = "https://jira.example.com/browse/";
pub const EMPTY: &str = "## Today\n\n## Later\n\n## Done\n";

/// A Jira link as `t` writes it.
pub fn l(key: &str) -> String {
    format!("[{key}]({URL}{key})")
}

/// The local date `days` days ago, as `YYYY-MM-DD`.
pub fn days_ago(days: u64) -> String {
    let d = chrono::Local::now().date_naive() - chrono::Days::new(days);
    d.format("%Y-%m-%d").to_string()
}

pub fn today() -> String {
    days_ago(0)
}

/// What a run of `t` did.
#[derive(Debug)]
pub struct Out {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    /// Exit status, stdout and stderr in one string, for compact checks.
    pub fn all(&self) -> String {
        format!("{}|{}{}", self.status, self.stdout, self.stderr)
    }
}

pub struct Sandbox {
    pub dir: tempfile::TempDir,
    pub home: PathBuf,
    /// The lists folder.
    pub lists: PathBuf,
    /// The default list, `TODO_FILE`.
    pub list: PathBuf,
    env: Vec<(String, String)>,
}

impl Sandbox {
    /// A sandbox whose default list holds `content`.
    pub fn new(content: &str) -> Sandbox {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let lists = dir.path().join("lists");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&lists).unwrap();
        let list = lists.join("Todo.md");
        fs::write(&list, content).unwrap();
        let mut s = Sandbox {
            dir,
            home,
            lists,
            list,
            env: Vec::new(),
        };
        s.set("TODO_FILE", &s.list.display().to_string());
        s.set("TODO_JIRA_KEYS", "PROJ OPS WEB");
        s.set("TODO_JIRA_URL", URL);
        s
    }

    /// Sets an environment variable for every later run.
    pub fn set(&mut self, name: &str, value: &str) {
        self.env.retain(|(n, _)| n != name);
        self.env.push((name.to_string(), value.to_string()));
    }

    /// Unsets an environment variable for every later run.
    pub fn unset(&mut self, name: &str) {
        self.env.retain(|(n, _)| n != name);
    }

    /// The config file `t` reads in this sandbox.
    pub fn config_file(&self) -> PathBuf {
        if cfg!(windows) {
            self.home.join("AppData").join("markdone").join("config")
        } else {
            self.home.join(".config").join("markdone").join("config")
        }
    }

    /// The command that runs `t` in this sandbox, for tests that need more control.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_t"));
        c.args(args).env_clear().current_dir(self.dir.path());
        for keep in ["PATH", "TZ", "SystemRoot", "PATHEXT", "SYSTEMDRIVE"] {
            if let Some(v) = std::env::var_os(keep) {
                c.env(keep, v);
            }
        }
        c.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("APPDATA", self.home.join("AppData"))
            .env("ZDOTDIR", &self.home);
        for (n, v) in &self.env {
            c.env(n, v);
        }
        c
    }

    /// Runs `t` with these arguments, stdin empty.
    pub fn t(&self, args: &[&str]) -> Out {
        self.t_with(args, &[])
    }

    /// Runs `t` with extra environment variables for this run only.
    pub fn t_with(&self, args: &[&str], env: &[(&str, &str)]) -> Out {
        let mut c = self.command(args);
        for (n, v) in env {
            c.env(n, v);
        }
        let Output {
            status,
            stdout,
            stderr,
        } = c.stdin(Stdio::null()).output().unwrap();
        Out {
            status: status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        }
    }

    /// The default list's contents.
    pub fn file(&self) -> String {
        read(&self.list)
    }

    /// Replaces the default list's contents.
    pub fn write(&self, content: &str) {
        fs::write(&self.list, content).unwrap();
    }
}

pub fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
