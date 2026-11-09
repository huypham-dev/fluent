//! Runs the Claude CLI and streams its text.

use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliErrorKind {
    NotFound,
    Unsupported,
    Failed,
    Timeout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CliError {
    pub kind: CliErrorKind,
    pub message: String,
}

impl CliError {
    fn new(kind: CliErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

// MARK: Stream parsing

#[derive(Debug, PartialEq)]
pub enum StreamEvent {
    Text(String),
    Completed,
    Failed(String),
    Ignored,
}

/// Parses one line of the Claude CLI's `stream-json` output.
pub fn parse_stream_line(line: &str) -> StreamEvent {
    let Ok(data) = serde_json::from_str::<Value>(line) else { return StreamEvent::Ignored };
    match data["type"].as_str() {
        Some("result") => {
            if data["is_error"] != Value::Bool(true) {
                return StreamEvent::Completed;
            }
            let message = data["result"].as_str().filter(|s| !s.is_empty());
            StreamEvent::Failed(message.unwrap_or("The Claude CLI reported an error.").to_string())
        }
        Some("stream_event") => {
            let event = &data["event"];
            match event["type"].as_str() {
                Some("message_stop") => StreamEvent::Completed,
                Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                    event["delta"]["text"].as_str().map_or(StreamEvent::Ignored, |t| StreamEvent::Text(t.into()))
                }
                _ => StreamEvent::Ignored,
            }
        }
        _ => StreamEvent::Ignored,
    }
}

/// Decodes only complete lines, so a multibyte character split across reads stays intact.
#[derive(Default)]
pub struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(newline) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=newline).collect();
            let text = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
            lines.push(text.strip_suffix('\r').map(str::to_string).unwrap_or(text));
        }
        lines
    }
}

// MARK: Running

pub struct CliOptions {
    pub executable: String,
    /// Arguments placed before the CLI flags. Tests use this to run a script.
    pub prefix_args: Vec<String>,
    pub model: String,
    pub system: String,
    pub prompt: String,
    pub cwd: PathBuf,
    pub inactivity: Duration,
}

pub enum CliEvent {
    Text(String),
    Done(Result<(), CliError>),
}

enum Message {
    Line(String),
    StdoutClosed,
    Cancel,
}

pub struct CliRun {
    sender: Sender<Message>,
}

impl CliRun {
    /// Stops the CLI. The run then reports success, which callers ignore for a cancelled run.
    pub fn cancel(&self) {
        let _ = self.sender.send(Message::Cancel);
    }
}

const EXIT_GRACE: Duration = Duration::from_millis(300);
const STDERR_LIMIT: usize = 8192;

fn expand_home(path: &str) -> PathBuf {
    let home = || dirs::home_dir().unwrap_or_default();
    if path == "~" {
        home()
    } else if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        home().join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// Apps started from Finder or at login do not inherit the shell's `PATH`.
fn search_path() -> std::ffi::OsString {
    let home = dirs::home_dir().unwrap_or_default();
    let mut paths = vec![home.join(".local").join("bin")];
    if !cfg!(windows) {
        paths.push("/opt/homebrew/bin".into());
        paths.push("/usr/local/bin".into());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).unwrap_or_default()
}

fn validate_executable(executable: &std::path::Path) -> Result<(), CliError> {
    let name = executable.to_string_lossy();
    let lower = name.to_lowercase();
    if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        return Err(CliError::new(
            CliErrorKind::Unsupported,
            format!("{name} is a script launcher, which is not supported. Use claude.exe."),
        ));
    }
    let not_found = || CliError::new(CliErrorKind::NotFound, format!("Claude CLI not found at {name}."));
    let metadata = std::fs::metadata(executable).map_err(|_| not_found())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(not_found());
        }
    }
    #[cfg(not(unix))]
    if !metadata.is_file() {
        return Err(not_found());
    }
    Ok(())
}

/// Runs the CLI once without a shell, so text can never be interpreted as a command.
/// `on_event` is called on a background thread and always ends with exactly one `Done`.
pub fn run_cli(options: CliOptions, mut on_event: impl FnMut(CliEvent) + Send + 'static) -> CliRun {
    let (sender, receiver) = mpsc::channel();
    let inner = sender.clone();
    thread::spawn(move || {
        let result = drive(options, inner, receiver, &mut on_event);
        on_event(CliEvent::Done(result));
    });
    CliRun { sender }
}

fn drive(
    options: CliOptions,
    sender: Sender<Message>,
    receiver: mpsc::Receiver<Message>,
    on_event: &mut impl FnMut(CliEvent),
) -> Result<(), CliError> {
    let executable = expand_home(&options.executable);
    validate_executable(&executable)?;
    let failed = |e: std::io::Error| CliError::new(CliErrorKind::Failed, e.to_string());

    // Dropped on return, which deletes the system prompt.
    let prompt_dir = tempfile::Builder::new().prefix("fluent-").tempdir().map_err(failed)?;
    let system_file = prompt_dir.path().join("system.txt");
    std::fs::write(&system_file, &options.system).map_err(failed)?;

    let mut command = Command::new(&executable);
    command
        .args(&options.prefix_args)
        .arg("-p")
        .arg("--system-prompt-file")
        .arg(&system_file)
        .args(["--tools", "", "--setting-sources", "", "--strict-mcp-config", "--no-session-persistence"])
        .args(["--model", &options.model, "--output-format", "stream-json", "--verbose"])
        .arg("--include-partial-messages")
        .current_dir(&options.cwd)
        .env("PATH", search_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(failed)?;

    // A CLI that exits early closes stdin; that surfaces when stdout closes.
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let prompt = options.prompt;
    thread::spawn(move || {
        let _ = stdin.write_all(prompt.as_bytes());
    });

    let mut stdout = child.stdout.take().expect("stdout is piped");
    thread::spawn(move || {
        let mut lines = LineBuffer::default();
        let mut chunk = [0u8; 8192];
        while let Ok(read) = stdout.read(&mut chunk) {
            if read == 0 {
                break;
            }
            for line in lines.push(&chunk[..read]) {
                if sender.send(Message::Line(line)).is_err() {
                    return;
                }
            }
        }
        let _ = sender.send(Message::StdoutClosed);
    });

    let stderr_text = Arc::new(Mutex::new(String::new()));
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let stderr_reader = {
        let stderr_text = stderr_text.clone();
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(read) = stderr.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                let mut text = stderr_text.lock().unwrap();
                text.push_str(&String::from_utf8_lossy(&chunk[..read]));
                if text.len() > STDERR_LIMIT {
                    let mut cut = text.len() - STDERR_LIMIT;
                    while !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    text.drain(..cut);
                }
            }
        })
    };

    let result = loop {
        match receiver.recv_timeout(options.inactivity) {
            Ok(Message::Line(line)) => match parse_stream_line(&line) {
                StreamEvent::Text(text) => on_event(CliEvent::Text(text)),
                StreamEvent::Completed => break Ok(()),
                StreamEvent::Failed(message) => break Err(CliError::new(CliErrorKind::Failed, message)),
                StreamEvent::Ignored => {}
            },
            Ok(Message::Cancel) => break Ok(()),
            Ok(Message::StdoutClosed) | Err(RecvTimeoutError::Disconnected) => {
                let status = wait_with_grace(&mut child);
                stop(&mut child);
                let _ = stderr_reader.join();
                let stderr = stderr_text.lock().unwrap().trim().to_string();
                let message = if !stderr.is_empty() {
                    stderr
                } else {
                    match status.and_then(|s| s.code()) {
                        Some(code) if code != 0 => format!("The Claude CLI exited with status {code}."),
                        _ => "The Claude CLI stopped before finishing.".to_string(),
                    }
                };
                break Err(CliError::new(CliErrorKind::Failed, message));
            }
            Err(RecvTimeoutError::Timeout) => {
                break Err(CliError::new(
                    CliErrorKind::Timeout,
                    format!("No response for {} seconds.", options.inactivity.as_secs_f64().round()),
                ));
            }
        }
    };
    stop(&mut child);
    result
}

fn wait_with_grace(child: &mut Child) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + EXIT_GRACE;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn delta(text: &str) -> String {
        json!({ "type": "stream_event", "event": { "type": "content_block_delta", "delta": { "type": "text_delta", "text": text } } }).to_string()
    }

    #[test]
    fn reads_a_text_delta() {
        assert_eq!(parse_stream_line(&delta("Hi")), StreamEvent::Text("Hi".into()));
    }

    #[test]
    fn treats_message_stop_and_success_as_completion() {
        assert_eq!(
            parse_stream_line(r#"{"type":"stream_event","event":{"type":"message_stop"}}"#),
            StreamEvent::Completed
        );
        assert_eq!(parse_stream_line(r#"{"type":"result","is_error":false}"#), StreamEvent::Completed);
    }

    #[test]
    fn reads_an_error_result() {
        assert_eq!(
            parse_stream_line(r#"{"type":"result","is_error":true,"result":"Not logged in"}"#),
            StreamEvent::Failed("Not logged in".into())
        );
        assert_eq!(
            parse_stream_line(r#"{"type":"result","is_error":true}"#),
            StreamEvent::Failed("The Claude CLI reported an error.".into())
        );
    }

    #[test]
    fn ignores_other_lines() {
        for line in ["", "not json", "null", r#"{"type":"system"}"#] {
            assert_eq!(parse_stream_line(line), StreamEvent::Ignored, "{line}");
        }
    }

    #[test]
    fn line_buffer_keeps_partial_lines() {
        let mut buffer = LineBuffer::default();
        assert_eq!(buffer.push(b"a\nb\nc"), ["a", "b"]);
        assert_eq!(buffer.push(b"\n"), ["c"]);
    }

    #[test]
    fn line_buffer_joins_a_split_multibyte_character() {
        let mut buffer = LineBuffer::default();
        let bytes = "sử dụng\n".as_bytes();
        assert!(buffer.push(&bytes[..2]).is_empty());
        assert_eq!(buffer.push(&bytes[2..]), ["sử dụng"]);
    }

    #[test]
    fn line_buffer_strips_carriage_returns() {
        assert_eq!(LineBuffer::default().push(b"a\r\n"), ["a"]);
    }

    #[cfg(unix)]
    mod process {
        use super::*;
        use std::fs;
        use std::path::Path;

        const STOP: &str = r#"{"type":"stream_event","event":{"type":"message_stop"}}"#;

        /// A fake CLI as a shell script. `$DIR` is its folder; `$@` holds the CLI flags.
        fn fake_cli(body: &str) -> (tempfile::TempDir, CliOptions) {
            let dir = tempfile::tempdir().unwrap();
            let script = dir.path().join("fake.sh");
            fs::write(&script, format!("DIR='{}'\n{body}\n", dir.path().display())).unwrap();
            let options = CliOptions {
                executable: "/bin/sh".into(),
                prefix_args: vec![script.to_string_lossy().into()],
                model: "sonnet".into(),
                system: "s".into(),
                prompt: "p".into(),
                cwd: dir.path().into(),
                inactivity: Duration::from_secs(10),
            };
            (dir, options)
        }

        fn quote(line: &str) -> String {
            format!("'{}'", line.replace('\'', "'\\''"))
        }

        /// Runs to completion, cancelling after the first text when `cancel_on_text` is set.
        fn collect(options: CliOptions, cancel_on_text: bool) -> (String, Result<(), CliError>) {
            let (tx, rx) = mpsc::channel();
            let run_slot: Arc<Mutex<Option<CliRun>>> = Arc::default();
            let slot = run_slot.clone();
            let run = run_cli(options, move |event| {
                if cancel_on_text && matches!(event, CliEvent::Text(_)) {
                    if let Some(run) = slot.lock().unwrap().as_ref() {
                        run.cancel();
                    }
                }
                let _ = tx.send(event);
            });
            *run_slot.lock().unwrap() = Some(run);
            let mut text = String::new();
            loop {
                match rx.recv_timeout(Duration::from_secs(15)).expect("the run finishes") {
                    CliEvent::Text(piece) => text.push_str(&piece),
                    CliEvent::Done(result) => return (text, result),
                }
            }
        }

        fn kind(result: Result<(), CliError>) -> CliErrorKind {
            result.unwrap_err().kind
        }

        #[test]
        fn streams_text_in_order_including_vietnamese() {
            let (_d, options) = fake_cli(&format!(
                "cat >/dev/null\necho {}\necho {}\necho {}",
                quote(&delta("Xin ")),
                quote(&delta("chào ử")),
                quote(STOP)
            ));
            assert_eq!(collect(options, false), ("Xin chào ử".into(), Ok(())));
        }

        #[test]
        fn finishes_on_message_stop_without_waiting_for_exit() {
            let (_d, options) = fake_cli(&format!("cat >/dev/null\necho {}\nsleep 20", quote(STOP)));
            let start = Instant::now();
            assert!(collect(options, false).1.is_ok());
            assert!(start.elapsed() < Duration::from_secs(5));
        }

        #[test]
        fn sends_the_prompt_over_stdin_and_the_expected_flags() {
            let (dir, mut options) = fake_cli(&format!(
                r#"cat > "$DIR/input.txt"
for arg in "$@"; do printf '%s\n' "$arg"; done > "$DIR/args.txt"
prev=''; for arg in "$@"; do if [ "$prev" = --system-prompt-file ]; then cp "$arg" "$DIR/system.txt"; fi; prev="$arg"; done
echo {}"#,
                quote(STOP)
            ));
            options.system = "SYSTEM ử".into();
            options.prompt = "the prompt".into();
            collect(options, false).1.unwrap();
            assert_eq!(fs::read_to_string(dir.path().join("input.txt")).unwrap(), "the prompt");
            assert_eq!(fs::read_to_string(dir.path().join("system.txt")).unwrap(), "SYSTEM ử");
            let args: Vec<String> =
                fs::read_to_string(dir.path().join("args.txt")).unwrap().lines().map(String::from).collect();
            assert!(!args.contains(&"the prompt".to_string()));
            for flag in
                ["-p", "--strict-mcp-config", "--no-session-persistence", "--verbose", "--include-partial-messages"]
            {
                assert!(args.iter().any(|a| a == flag), "{flag}");
            }
            let value = |flag: &str| args[args.iter().position(|a| a == flag).unwrap() + 1].clone();
            assert_eq!(value("--tools"), "");
            assert_eq!(value("--setting-sources"), "");
            assert_eq!(value("--model"), "sonnet");
            assert_eq!(value("--output-format"), "stream-json");
        }

        fn system_file_path(dir: &Path) -> String {
            fs::read_to_string(dir.join("path.txt")).unwrap().trim().to_string()
        }

        const SAVE_PATH: &str = r#"prev=''; for arg in "$@"; do if [ "$prev" = --system-prompt-file ]; then echo "$arg" > "$DIR/path.txt"; fi; prev="$arg"; done"#;

        #[test]
        fn deletes_the_system_prompt_after_success_failure_and_cancel() {
            for (finish, cancel) in [
                (format!("echo {}", quote(STOP)), false),
                ("exit 1".to_string(), false),
                (format!("echo {}\nsleep 20", quote(&delta("x"))), true),
            ] {
                let (dir, options) = fake_cli(&format!("cat >/dev/null\n{SAVE_PATH}\n{finish}"));
                let _ = collect(options, cancel);
                assert!(!Path::new(&system_file_path(dir.path())).exists());
            }
        }

        #[test]
        fn reports_a_missing_executable() {
            let (_d, mut options) = fake_cli("");
            options.executable = "/nonexistent/claude".into();
            options.prefix_args.clear();
            assert_eq!(kind(collect(options, false).1), CliErrorKind::NotFound);
        }

        #[test]
        fn rejects_script_launchers() {
            for executable in ["C:\\tools\\claude.cmd", "C:\\tools\\CLAUDE.BAT"] {
                let (_d, mut options) = fake_cli("");
                options.executable = executable.into();
                assert_eq!(kind(collect(options, false).1), CliErrorKind::Unsupported);
            }
        }

        #[test]
        fn reports_a_non_zero_exit_with_stderr() {
            let (_d, options) = fake_cli("cat >/dev/null\necho 'boom happened' >&2\nexit 3");
            let error = collect(options, false).1.unwrap_err();
            assert_eq!(error.kind, CliErrorKind::Failed);
            assert!(error.message.contains("boom happened"));
        }

        #[test]
        fn reports_an_error_result_and_keeps_earlier_text() {
            let failure = r#"{"type":"result","is_error":true,"result":"Not logged in"}"#;
            let (_d, options) =
                fake_cli(&format!("cat >/dev/null\necho {}\necho {}", quote(&delta("partial")), quote(failure)));
            let (text, result) = collect(options, false);
            assert_eq!(text, "partial");
            assert_eq!(result.unwrap_err().message, "Not logged in");
        }

        #[test]
        fn treats_an_exit_without_completion_as_failure() {
            let (_d, options) = fake_cli(&format!("cat >/dev/null\necho {}", quote(&delta("partial"))));
            assert_eq!(kind(collect(options, false).1), CliErrorKind::Failed);
        }

        #[test]
        fn times_out_on_silence() {
            let (_d, mut options) = fake_cli("sleep 20");
            options.inactivity = Duration::from_millis(300);
            assert_eq!(kind(collect(options, false).1), CliErrorKind::Timeout);
        }

        #[test]
        fn resets_the_timeout_whenever_output_arrives() {
            let (_d, mut options) = fake_cli(&format!(
                "cat >/dev/null\nfor i in 1 2 3 4 5 6; do sleep 0.2; echo {}; done\necho {}",
                quote(&delta("a")),
                quote(STOP)
            ));
            options.inactivity = Duration::from_millis(700);
            assert_eq!(collect(options, false), ("aaaaaa".into(), Ok(())));
        }

        #[test]
        fn terminates_the_process_on_cancel() {
            let (dir, options) =
                fake_cli(&format!("cat >/dev/null\necho $$ > \"$DIR/pid.txt\"\necho {}\nsleep 20", quote(&delta("x"))));
            assert!(collect(options, true).1.is_ok());
            let pid = fs::read_to_string(dir.path().join("pid.txt")).unwrap().trim().to_string();
            let alive = Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success();
            assert!(!alive);
        }

        #[test]
        fn survives_a_process_that_exits_before_reading_a_large_prompt() {
            let (_d, mut options) = fake_cli("exit 1");
            options.prompt = "x".repeat(2_000_000);
            assert_eq!(kind(collect(options, false).1), CliErrorKind::Failed);
        }
    }

    /// Calls the real Claude CLI and uses real usage. Run with
    /// `FLUENT_LIVE=1 cargo test live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_cli() {
        use crate::actions::ActionId;
        use crate::prompt::{Languages, system_prompt, user_prompt};
        if std::env::var("FLUENT_LIVE").as_deref() != Ok("1") {
            return;
        }
        let home = dirs::home_dir().unwrap().to_string_lossy().into_owned();
        let languages = Languages { primary: "Vietnamese", secondary: "English" };
        for (action, selected) in [
            (ActionId::Draft, "Can you review my PR today?"),
            (ActionId::Explain, "The deployment failed because of a race condition in the migration."),
            (ActionId::Translate, "Please let me know when the deployment is finished."),
            (ActionId::Translate, "Bạn gửi link PR giúp mình nhé."),
            (ActionId::Translate, "Mình đã deploy bản fix lên staging rồi, bạn test lại rồi merge PR giúp mình nhé."),
        ] {
            let (tx, rx) = mpsc::channel();
            let start = Instant::now();
            let _run = run_cli(
                CliOptions {
                    executable: crate::settings::default_cli_path(cfg!(windows), &home),
                    prefix_args: vec![],
                    model: "sonnet".into(),
                    system: system_prompt(action, &languages, None),
                    prompt: user_prompt(selected, None),
                    cwd: std::env::temp_dir(),
                    inactivity: Duration::from_secs(60),
                },
                move |event| {
                    let _ = tx.send(event);
                },
            );
            let mut text = String::new();
            loop {
                match rx.recv().unwrap() {
                    CliEvent::Text(piece) => text.push_str(&piece),
                    CliEvent::Done(result) => {
                        result.unwrap();
                        break;
                    }
                }
            }
            println!("LIVE {} {}ms: {text}", action.key(), start.elapsed().as_millis());
            assert!(!text.trim().is_empty());
        }
    }
}
