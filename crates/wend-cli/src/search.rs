//! Answers the editor's searches by running the command of `[search]` for what is searched, which
//! is the one place wend finds files and text: it has no matcher or walker of its own.
//!
//! The command runs through `sh -c`, so this is Unix only for now. The query is typed text, so
//! it goes in the environment variable `WEND_QUERY` and is never written into the command line.

use std::{
    io::{self, BufRead, BufReader, Read},
    os::unix::process::CommandExt,
    process::{Child, ChildStderr, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
};

use wend_core::{
    config,
    input::{Failure, Input, action::SearchKind},
};

/// The most lines of an answer. The command is stopped after them, as nobody reads further
/// down than a screen, and a command that lists a whole disk would never end.
const LINE_LIMIT: usize = 1000;

/// The most bytes of what the command said on its error output that is kept to say why it failed.
const MESSAGE_LIMIT: u64 = 4096;

/// Runs one search at a time on a thread of its own, and stops it when a newer one is asked for.
pub(crate) struct Search {
    /// The command that finds files, and the one that finds text.
    files: Arc<str>,
    text: Arc<str>,
    input: mpsc::Sender<Input>,
    running: Option<Arc<Job>>,
}

/// A search that is running, which the next search stops.
#[derive(Default)]
struct Job {
    cancelled: AtomicBool,
    /// The process id of the command, which is also the id of its process group. Zero until it
    /// has started.
    process: AtomicU32,
}

impl Job {
    /// Stops the command and everything it started, so a `fd | fzf` pipe doesn't leave `fd`
    /// behind. A command that hasn't started yet is stopped by the thread that starts it.
    ///
    /// The kill runs on a thread of its own, since this is called from the loop that draws the
    /// frames and waiting for `kill` there would hold up the screen on every key typed.
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let process = self.process.load(Ordering::SeqCst);
        if process != 0 {
            thread::spawn(move || kill_group(process));
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Search {
    pub(crate) fn new(config: &config::Search, input: mpsc::Sender<Input>) -> Self {
        Self::with_commands(&config.files, &config.text, input)
    }

    fn with_commands(files: &str, text: &str, input: mpsc::Sender<Input>) -> Self {
        Self {
            files: files.into(),
            text: text.into(),
            input,
            running: None,
        }
    }

    /// Stops the search that is running and starts search `id` of `kind` for `query`. Its
    /// answer, or the reason it failed, is sent to the editor as input.
    pub(crate) fn start(&mut self, id: u64, kind: SearchKind, query: &str) {
        if let Some(job) = self.running.take() {
            job.cancel();
        }
        let job = Arc::new(Job::default());
        self.running = Some(Arc::clone(&job));
        let command = Arc::clone(match kind {
            SearchKind::Files => &self.files,
            SearchKind::Text => &self.text,
        });
        let query = query.to_owned();
        let input = self.input.clone();
        thread::spawn(move || {
            if let Some(answer) = run(&job, id, &command, &query) {
                // The editor is gone when this fails, and nobody is left to hear it
                drop(input.send(answer));
            }
        });
    }
}

/// Runs `command` for `query` and returns what to tell the editor, nothing when the search was
/// stopped, since a newer one has taken its place.
fn run(job: &Job, id: u64, command: &str, query: &str) -> Option<Input> {
    let mut child = match Command::new("sh")
        .args(["-c", command])
        .env("WEND_QUERY", query)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Its own group, to stop the whole pipeline at once
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return Some(failed(id, &format!("cannot run sh: {error}"))),
    };
    job.process.store(child.id(), Ordering::SeqCst);
    // Cancelled while it was starting, which `cancel` couldn't stop yet
    if job.is_cancelled() {
        kill_group(child.id());
    }

    // Read while the lines are, since a command that says more than the pipe holds would wait
    // for someone to read it, and never end
    let message = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_message(stderr)));
    let (text, full) = read_lines(&mut child);
    if full {
        kill_group(child.id());
    }
    let status = child.wait();
    let message = message
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    if job.is_cancelled() {
        return None;
    }
    Some(match status {
        Ok(status) => answer(id, text, status, full, &message),
        Err(error) => failed(id, &error.to_string()),
    })
}

/// Reads the output of the command, up to `LINE_LIMIT` lines. Says whether it stopped there
/// with more to come.
fn read_lines(child: &mut Child) -> (String, bool) {
    let Some(stdout) = child.stdout.take() else {
        return (String::new(), false);
    };
    let mut reader = BufReader::new(stdout);
    let mut text = String::new();
    let mut line = vec![];
    for _ in 0..LINE_LIMIT {
        line.clear();
        // What isn't text is a line the editor can't open anyway
        if !matches!(reader.read_until(b'\n', &mut line), Ok(read) if read > 0) {
            return (text, false);
        }
        text.push_str(&String::from_utf8_lossy(&line));
    }
    (text, true)
}

/// What the command said on its error output, the last of the lines that fit in `MESSAGE_LIMIT`.
/// What comes after is read and thrown away, so the command never waits on it.
fn read_message(mut stderr: ChildStderr) -> String {
    let mut bytes = vec![];
    // An error output that is cut off may end in the middle of a character
    drop((&mut stderr).take(MESSAGE_LIMIT).read_to_end(&mut bytes));
    drop(io::copy(&mut stderr, &mut io::sink()));
    String::from_utf8_lossy(&bytes)
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The editor's input for a command that ended with `status` after printing `text`. One that
/// found nothing may end with the error status 1, as `fzf --filter` does, so that alone is no
/// failure. What it said on its error output is one when it printed nothing, or ended badly.
/// A command that was stopped at the line limit failed at nothing.
fn answer(id: u64, text: String, status: ExitStatus, full: bool, message: &str) -> Input {
    if !full {
        if !message.is_empty() && (text.is_empty() || !status.success()) {
            return failed(id, message);
        }
        if !status.success() && status.code() != Some(1) {
            return failed(id, &format!("the command ended with {status}"));
        }
    }
    Input::SearchResults {
        id,
        text: text.into(),
    }
}

fn failed(id: u64, reason: &str) -> Input {
    Input::Failed(Failure::Search {
        id,
        reason: reason.into(),
    })
}

/// Stops the process group `process`. It goes through `kill`, as signaling a group takes
/// `unsafe` code that the project doesn't allow.
fn kill_group(process: u32) {
    drop(
        Command::new("kill")
            .args(["-KILL", "--"])
            .arg(format!("-{process}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Runs `command` for `query` and waits for the answer.
    fn searched(command: &str, query: &str) -> Input {
        let (sender, receiver) = mpsc::channel();
        Search::with_commands(command, command, sender).start(1, SearchKind::Files, query);
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the search answers")
    }

    fn results(input: &Input) -> &str {
        match input {
            Input::SearchResults { text, .. } => text,
            other => panic!("not results: {other:?}"),
        }
    }

    #[test]
    fn the_query_reaches_the_command_in_the_environment() {
        let input = searched(r#"printf '%s\n' "$WEND_QUERY""#, "a b; echo hi");

        assert_eq!(
            results(&input),
            "a b; echo hi\n",
            "it is never part of the command"
        );
    }

    #[test]
    fn a_command_that_finds_nothing_with_an_error_status_is_no_results() {
        let input = searched("exit 1", "");

        assert_eq!(results(&input), "");
    }

    #[test]
    fn a_command_that_is_not_installed_is_a_failure_with_its_words() {
        let input = searched("wend-no-such-tool | cat", "");

        let Input::Failed(Failure::Search { id, reason }) = input else {
            panic!("not a failure: {input:?}");
        };
        assert_eq!(id, 1);
        assert!(reason.contains("wend-no-such-tool"), "{reason}");
    }

    #[test]
    fn a_command_that_says_a_lot_on_its_error_output_still_ends() {
        let input = searched("yes error | head -c 400000 >&2; echo found", "");

        assert_eq!(
            results(&input),
            "found\n",
            "more than a pipe holds on stderr doesn't leave the command waiting"
        );
    }

    #[test]
    fn a_command_that_never_ends_is_stopped_after_the_line_limit() {
        let input = searched("yes", "");

        assert_eq!(results(&input).lines().count(), LINE_LIMIT);
    }

    #[test]
    fn a_newer_search_stops_the_older_one() {
        let (sender, receiver) = mpsc::channel();
        let mut search = Search::with_commands("sleep 30; echo late", "", sender);
        search.start(1, SearchKind::Files, "");
        thread::sleep(Duration::from_millis(100));

        search.files = "echo now".into();
        search.start(2, SearchKind::Files, "");

        let input = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            matches!(input, Input::SearchResults { id: 2, .. }),
            "{input:?}"
        );
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "the first search says nothing"
        );
    }
}
