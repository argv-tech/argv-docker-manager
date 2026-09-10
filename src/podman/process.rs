use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

use crate::service::SharedLogBuffer;

type LineCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

pub fn run_capture(mut cmd: Command) -> std::io::Result<Output> {
    cmd.output()
}

// Called by the blocking service-operation workers.
pub fn run_stream_with_line_callback(
    cmd: Command,
    logs: SharedLogBuffer,
    header: Option<&str>,
    on_line: Option<LineCallback>,
) -> std::io::Result<bool> {
    tokio::runtime::Handle::current().block_on(async move {
        let mut cmd = tokio::process::Command::from(cmd);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn()?;
        if let Some(header) = header {
            logs.lock().unwrap().push_str(header);
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (status, stdout_result, stderr_result) = tokio::join!(
            child.wait(),
            read_output(stdout, &logs, on_line.as_ref()),
            read_output(stderr, &logs, on_line.as_ref()),
        );
        stdout_result?;
        stderr_result?;
        Ok(status?.success())
    })
}

async fn read_output<R: AsyncRead + Unpin>(
    stream: Option<R>,
    logs: &SharedLogBuffer,
    on_line: Option<&LineCallback>,
) -> std::io::Result<()> {
    let Some(stream) = stream else {
        return Ok(());
    };
    let mut lines = BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await? {
        logs.lock().unwrap().push_line(&line);
        if let Some(callback) = on_line {
            callback(&line);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::LogBuffer;
    use std::sync::Mutex;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn command_completion_waits_for_output_and_callbacks() {
        let logs = Arc::new(Mutex::new(LogBuffer::command()));
        let callback_logs = Arc::clone(&logs);
        let output_logs = Arc::clone(&logs);
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf 'port already in use\\n' >&2; exit 1"]);
        let success = tokio::task::spawn_blocking(move || {
            run_stream_with_line_callback(
                cmd,
                output_logs,
                None,
                Some(Arc::new(move |_| {
                    // A callback may access the output buffer without deadlocking.
                    callback_logs.lock().unwrap().push_line("callback finished");
                })),
            )
        })
        .await
        .unwrap()
        .unwrap();
        assert!(!success);
        assert_eq!(
            logs.lock().unwrap().as_str(),
            "port already in use\ncallback finished\n"
        );
    }
}
