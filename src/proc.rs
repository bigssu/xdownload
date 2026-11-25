//! Runs yt-dlp / ffmpeg without a console window, streams their output by line and lets the
//! UI cancel them from another thread.
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};

/// Keeps a console window from flashing up for every child process.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Exit code reported for a cancelled run.
pub const CANCELLED: i32 = -1;

/// A command that never shows a console window and never waits on stdin.
pub fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).creation_flags(CREATE_NO_WINDOW);
    command
}

/// One unit of background work (a batch of downloads or conversions) that can be cancelled.
#[derive(Default)]
pub struct Job {
    cancel: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl Job {
    pub const fn new() -> Self {
        Self { cancel: AtomicBool::new(false), child: Mutex::new(None) }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Stops the job and kills the running process together with its children (yt-dlp starts
    /// its own ffmpeg, which would otherwise keep running and hold the output file).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            kill_tree(child);
        }
    }
}

fn kill_tree(child: &mut Child) {
    let killed = command("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success());
    if !killed {
        let _ = child.kill();
    }
}

/// Runs `program` and calls `on_line` for every non-empty output line (stdout and stderr).
/// Returns the exit code, or `CANCELLED` when the job was cancelled.
pub fn run(program: &str, args: &[String], job: &Job, mut on_line: impl FnMut(&str)) -> std::io::Result<i32> {
    if job.cancelled() {
        return Ok(CANCELLED);
    }
    let mut child = command(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let (tx, rx) = mpsc::channel::<String>();
    // Both pipes are drained on their own threads: a full, unread pipe would block the child.
    let readers = [
        child.stdout.take().map(|pipe| spawn_reader(pipe, tx.clone())),
        child.stderr.take().map(|pipe| spawn_reader(pipe, tx.clone())),
    ];
    drop(tx);
    *job.child.lock().unwrap() = Some(child);
    // A cancel that arrived while the process was starting found no child to kill.
    if job.cancelled() {
        job.cancel();
    }
    for line in rx {
        on_line(&line);
    }
    for reader in readers.into_iter().flatten() {
        let _ = reader.join();
    }
    let child = job.child.lock().unwrap().take();
    let status = match child {
        Some(mut child) => child.wait()?,
        None => return Ok(CANCELLED),
    };
    Ok(if job.cancelled() { CANCELLED } else { status.code().unwrap_or(1) })
}

fn spawn_reader(pipe: impl Read + Send + 'static, tx: mpsc::Sender<String>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut buffer = Vec::new();
        // Progress updates may end in a bare carriage return, so split on both.
        while matches!(reader.read_until(b'\n', &mut buffer), Ok(n) if n > 0) {
            for part in String::from_utf8_lossy(&buffer).split(['\r', '\n']) {
                let part = part.trim();
                if !part.is_empty() && tx.send(part.to_owned()).is_err() {
                    return;
                }
            }
            buffer.clear();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn output_lines_from_both_pipes_and_the_exit_code_are_reported() {
        let mut lines = Vec::new();
        let code = run("cmd", &args(&["/c", "echo one& echo two 1>&2& exit 3"]), &Job::default(), |line| lines.push(line.to_owned())).unwrap();
        lines.sort();
        assert_eq!(lines, ["one", "two"]);
        assert_eq!(code, 3);
        assert!(run("xdownload2-no-such-program", &[], &Job::default(), |_| {}).is_err());
    }

    #[test]
    fn cancel_kills_the_process_tree_and_reports_cancelled() {
        let job = Arc::new(Job::default());
        let canceller = job.clone();
        let started = std::time::Instant::now();
        // The inner cmd is a grandchild, like the ffmpeg that yt-dlp starts.
        let code = run("cmd", &args(&["/c", "echo ready& cmd /c ping -n 30 127.0.0.1"]), &job, |line| {
            if line == "ready" {
                let canceller = canceller.clone();
                std::thread::spawn(move || canceller.cancel());
            }
        }).unwrap();
        assert_eq!(code, CANCELLED);
        assert!(started.elapsed() < std::time::Duration::from_secs(15), "the tree must not run to completion");
        assert_eq!(run("cmd", &args(&["/c", "echo never"]), &job, |_| panic!("a cancelled job starts nothing")).unwrap(), CANCELLED);
    }
}
