use std::io::{BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::thread;

use crate::service::SharedLogBuffer;

type LineCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

pub fn run_capture(mut cmd: Command) -> std::io::Result<Output> {
    cmd.output()
}

pub fn run_stream_with_line_callback(
    mut cmd: Command,
    logs: SharedLogBuffer,
    header: Option<&str>,
    on_line: Option<LineCallback>,
) -> std::io::Result<bool> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    if let Some(header) = header {
        let mut logs_lock = logs.lock().unwrap();
        logs_lock.push_str(header);
    }

    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let logs_stdout = Arc::clone(&logs);
        let on_line_stdout = on_line.clone();
        readers.push(thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                logs_stdout.lock().unwrap().push_line(&line);
                if let Some(callback) = &on_line_stdout {
                    callback(&line);
                }
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let logs_stderr = Arc::clone(&logs);
        let on_line_stderr = on_line;
        readers.push(thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                logs_stderr.lock().unwrap().push_line(&line);
                if let Some(callback) = &on_line_stderr {
                    callback(&line);
                }
            }
        }));
    }

    let status = child.wait()?;
    for reader in readers {
        reader
            .join()
            .map_err(|_| std::io::Error::other("command output reader panicked"))?;
    }
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::LogBuffer;
    use std::sync::Mutex;

    #[test]
    fn command_completion_waits_for_output_and_callbacks() {
        let logs = Arc::new(Mutex::new(LogBuffer::command()));
        let callback_logs = Arc::clone(&logs);
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf 'port already in use\\n' >&2; exit 1"]);
        let success = run_stream_with_line_callback(
            cmd,
            Arc::clone(&logs),
            None,
            Some(Arc::new(move |_| {
                // A callback may access the output buffer without deadlocking.
                callback_logs.lock().unwrap().push_line("callback finished");
            })),
        )
        .unwrap();
        assert!(!success);
        assert_eq!(
            logs.lock().unwrap().as_str(),
            "port already in use\ncallback finished\n"
        );
    }
}
