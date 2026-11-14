//! One run at a time (0.3.0's "locking" section). The holding side is this test process, with the
//! standard library's lock, as another `t` would take it.

mod common;
use common::*;
use std::fs::{File, OpenOptions};
use std::time::{Duration, Instant};

fn hold(path: &std::path::Path) -> File {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    f.lock().unwrap();
    f
}

#[test]
fn parallel_adds_lose_nothing() {
    let s = Sandbox::new(EMPTY);
    let children: Vec<_> = (0..20)
        .map(|i| {
            s.command(&["add", &format!("parallel {i}")])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut c in children {
        assert!(c.wait().unwrap().success());
    }
    assert_eq!(s.file().matches("- [ ] parallel ").count(), 20);
}

#[test]
fn waits_for_a_lock_that_is_released() {
    let s = Sandbox::new(EMPTY);
    let held = hold(&s.lists.join(".Todo.md.lock"));
    let child = s
        .command(&["add", "waited"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    drop(held);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Added 1 (Today): waited\n"
    );
}

#[test]
fn gives_up_on_a_lock_that_is_held() {
    let s = Sandbox::new(EMPTY);
    let _held = hold(&s.lists.join(".Todo.md.lock"));
    let before = s.file();
    let start = Instant::now();
    assert_eq!(
        s.t(&["add", "blocked"]).all(),
        "1|t: another t has been using the list for 10 seconds; try again when it's done\n"
    );
    assert!(start.elapsed() >= Duration::from_secs(9));
    assert_eq!(s.file(), before);
}

#[test]
fn help_and_lists_dont_lock_the_list() {
    let s = Sandbox::new(EMPTY);
    s.t(&["help"]);
    s.t(&["lists"]);
    assert!(!s.lists.join(".Todo.md.lock").exists());
}

#[cfg(unix)]
#[test]
fn an_unwritable_folder_cant_be_locked() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new(EMPTY);
    std::fs::set_permissions(&s.lists, std::fs::Permissions::from_mode(0o555)).unwrap();
    let out = s.t(&[]);
    std::fs::set_permissions(&s.lists, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        out.all(),
        format!(
            "1|t: could not create the lock file {}\n",
            s.lists.join(".Todo.md.lock").display()
        )
    );
}

/// Not a test of its own: when T_HOLD names a lock file, takes that lock, creates T_READY and
/// sleeps, so another test can kill this process while it holds the lock.
#[test]
fn holder_helper() {
    let (Some(lock), Some(ready)) = (std::env::var_os("T_HOLD"), std::env::var_os("T_READY"))
    else {
        return;
    };
    let _held = hold(std::path::Path::new(&lock));
    std::fs::write(ready, b"").unwrap();
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn the_lock_of_a_killed_process_is_free_at_once() {
    let s = Sandbox::new(EMPTY);
    let ready = s.dir.path().join("ready");
    let mut holder = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "holder_helper", "--nocapture"])
        .env("T_HOLD", s.lists.join(".Todo.md.lock"))
        .env("T_READY", &ready)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    while !ready.exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
    holder.kill().unwrap();
    holder.wait().unwrap();
    let start = Instant::now();
    assert_eq!(s.t(&["add", "freed"]).all(), "0|Added 1 (Today): freed\n");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_named_list_works_while_the_default_list_is_locked() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Job"]);
    let _held = hold(&s.lists.join(".Todo.md.lock"));
    let start = Instant::now();
    assert_eq!(
        s.t(&["@Job", "add", "quick"]).all(),
        "0|Added 1 (Today): quick\n"
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn move_waits_for_the_target_lists_lock() {
    let s = Sandbox::new("## Today\n- [ ] Buy milk\n\n## Done\n");
    s.t(&["lists", "add", "Work"]);
    let held = hold(&s.lists.join(".Work.md.lock"));
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        drop(held);
    });
    let start = Instant::now();
    assert_eq!(
        s.t(&["move", "milk", "@Work"]).all(),
        "0|Moved to Work (Today) as 1: Buy milk\n"
    );
    assert!(start.elapsed() > Duration::from_millis(500), "it waited");
    release.join().unwrap();
}
