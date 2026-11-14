//! One run at a time: a lock on an empty lock file next to a list (or in the lists folder), held
//! until the process ends. See docs/designs/rust_port.md, "Locking" and "Dispatch" (one lock per
//! file per run).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

/// What a lock protects, which only changes the timeout message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// A list (`.<file>.lock`).
    List,
    /// The set of lists (`.lists.lock` in the lists folder).
    Folder,
}

/// A held lock. The kernel releases it when this is dropped or the process ends, however it ends.
/// The standard library opens files close-on-exec on Unix and non-inheritable on Windows, so fzf
/// and the editor never hold it.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

/// How long to wait for another run, and how often to check (0.3.0's values).
pub const TIMEOUT: Duration = Duration::from_secs(10);
const INTERVAL: Duration = Duration::from_millis(50);

/// Takes the lock on `path`, creating the empty lock file when needed, waiting up to 10 s.
pub fn take(path: &Path, what: What) -> Result<Lock, String> {
    take_within(path, what, TIMEOUT)
}

/// `take` with another timeout, for tests.
pub fn take_within(path: &Path, what: What, timeout: Duration) -> Result<Lock, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| format!("could not create the lock file {}", path.display()))?;
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Lock { _file: file }),
            Err(TryLockError::WouldBlock) if start.elapsed() < timeout => {
                std::thread::sleep(INTERVAL)
            }
            Err(TryLockError::WouldBlock) => {
                return Err(match what {
                    What::List => {
                        "another t has been using the list for 10 seconds; try again when it's done"
                    }
                    What::Folder => {
                        "another t has been changing the lists for 10 seconds; try again when it's done"
                    }
                }
                .to_string());
            }
            Err(TryLockError::Error(_)) => {
                return Err(format!("could not lock {}", path.display()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_lock_makes_others_wait_and_give_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".Todo.md.lock");
        let held = take(&path, What::List).unwrap();
        let short = Duration::from_millis(150);
        let start = Instant::now();
        // A second handle, even in the same process, doesn't get it: one lock per file per run.
        assert_eq!(
            take_within(&path, What::List, short).unwrap_err(),
            "another t has been using the list for 10 seconds; try again when it's done"
        );
        assert!(start.elapsed() >= short);
        assert_eq!(
            take_within(&path, What::Folder, short).unwrap_err(),
            "another t has been changing the lists for 10 seconds; try again when it's done"
        );
        drop(held);
        assert!(
            take_within(&path, What::List, short).is_ok(),
            "free once released"
        );
    }

    #[test]
    fn the_lock_file_is_created_empty_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".Todo.md.lock");
        drop(take(&path, What::List).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        std::fs::write(&path, b"left alone").unwrap();
        drop(take(&path, What::List).unwrap());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"left alone",
            "never truncated"
        );
    }

    #[test]
    fn a_lock_file_that_cant_be_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join(".Todo.md.lock");
        assert_eq!(
            take(&path, What::List).unwrap_err(),
            format!("could not create the lock file {}", path.display())
        );
    }
}
