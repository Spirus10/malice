//! Stdio client for worker-style plugin runtimes.
//!
//! Process model:
//!
//!   teamserver
//!      -> WorkerPluginClient
//!      -> child process stdin/stdout/stderr
//!      -> plugin runtime
//!
//! Request/response flow:
//!
//!   Rust request struct
//!      -> JSON line on stdin
//!      -> plugin handles `operation`
//!      -> JSON line on stdout
//!      -> typed Rust response
//!
//! Stderr is drained on a background thread and only surfaced when protocol
//! errors occur, so plugin diagnostics are preserved without mixing them into
//! the request channel.

use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Result, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    package::{PluginPackageDescriptor, PluginRuntimeDescriptor},
    plugin_api::{request_id, PluginRequestEnvelope, PluginResponseEnvelope},
};

#[derive(Debug)]
pub struct WorkerPluginClient {
    session: Mutex<WorkerSession>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Debug)]
struct WorkerSession {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl WorkerPluginClient {
    /// Starts the plugin worker process declared by the package descriptor.
    ///
    /// This is where the runtime boundary is established: the configured command
    /// is resolved relative to the package, the child is spawned, and stderr is
    /// drained into a ring buffer so later protocol failures can include plugin
    /// diagnostics.
    pub fn start(package_root: &Path, descriptor: &PluginPackageDescriptor) -> Result<Self> {
        let PluginRuntimeDescriptor {
            runtime_type,
            command,
        } = &descriptor.runtime;
        if runtime_type != "stdio" {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "Plugin '{}' uses unsupported runtime type '{}'",
                    descriptor.plugin_id, runtime_type
                ),
            ));
        }
        let (program, args) = resolve_command(package_root, command)?;
        let mut child = Command::new(&program)
            .args(args)
            .current_dir(package_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                Error::new(
                    err.kind(),
                    format!(
                        "Unable to start plugin '{}' using '{}': {err}",
                        descriptor.plugin_id,
                        program.display()
                    ),
                )
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            Error::other(format!(
                "Plugin '{}' did not provide stdin",
                descriptor.plugin_id
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            Error::other(format!(
                "Plugin '{}' did not provide stdout",
                descriptor.plugin_id
            ))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            Error::other(format!(
                "Plugin '{}' did not provide stderr",
                descriptor.plugin_id
            ))
        })?;
        let stderr_lines = Arc::new(Mutex::new(VecDeque::with_capacity(32)));
        spawn_stderr_drain(stderr, stderr_lines.clone());

        Ok(Self {
            session: Mutex::new(WorkerSession {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
            }),
            stderr_lines,
        })
    }

    /// Sends one RPC-style operation to the plugin and waits for the reply.
    ///
    /// Protocol contract:
    ///
    ///   request_id generated here
    ///      -> request JSON line written to stdin
    ///      -> response JSON line read from stdout
    ///      -> request_id must match
    ///      -> payload is deserialized into `T`
    ///
    /// Any recent stderr lines are appended to returned errors so a malformed
    /// response can still be tied back to plugin-side failures.
    pub fn call<T: DeserializeOwned>(&self, operation: &str, payload: Value) -> Result<T> {
        let request = PluginRequestEnvelope {
            request_id: request_id(),
            operation: operation.to_string(),
            payload,
        };
        let mut session = self.session.lock().expect("worker session lock poisoned");
        write_json_line(&mut session.stdin, &request)?;
        let response: PluginResponseEnvelope =
            read_json_line(&mut session.stdout).map_err(|err| self.decorate_protocol_error(err))?;
        if response.request_id != request.request_id {
            return Err(self.decorate_protocol_error(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Response request id '{}' did not match '{}'",
                    response.request_id, request.request_id
                ),
            )));
        }
        if !response.ok {
            return Err(self.decorate_protocol_error(Error::other(
                response
                    .error
                    .unwrap_or_else(|| "plugin reported an unknown failure".to_string()),
            )));
        }
        let payload = response.payload.ok_or_else(|| {
            self.decorate_protocol_error(Error::new(
                ErrorKind::InvalidData,
                "Plugin response missing payload",
            ))
        })?;
        serde_json::from_value(payload).map_err(|err| {
            self.decorate_protocol_error(Error::new(
                ErrorKind::InvalidData,
                format!("Unable to parse plugin response payload: {err}"),
            ))
        })
    }

    /// Async wrapper around `call()` that runs the blocking I/O on a dedicated
    /// thread pool via `spawn_blocking` and enforces a 10-second timeout.
    pub async fn call_async<T: serde::de::DeserializeOwned + Send + 'static>(
        self: &Arc<Self>,
        operation: &str,
        payload: serde_json::Value,
    ) -> Result<T> {
        let client = Arc::clone(self);
        let operation = operation.to_string();
        let handle = tokio::task::spawn_blocking(move || client.call::<T>(&operation, payload));
        match tokio::time::timeout(Duration::from_secs(10), handle).await {
            Ok(Ok(result)) => result,
            Ok(Err(join_err)) => Err(Error::other(format!("worker task panicked: {join_err}"))),
            Err(_elapsed) => Err(Error::new(
                ErrorKind::TimedOut,
                "plugin worker call timed out after 10s",
            )),
        }
    }

    /// Attaches recent plugin stderr output to a protocol error.
    fn decorate_protocol_error(&self, err: Error) -> Error {
        let stderr = self
            .stderr_lines
            .lock()
            .expect("stderr lock poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        if stderr.is_empty() {
            err
        } else {
            Error::new(err.kind(), format!("{} | stderr: {}", err, stderr))
        }
    }
}

impl Drop for WorkerPluginClient {
    fn drop(&mut self) {
        if let Ok(mut session) = self.session.lock() {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
    }
}

fn resolve_command(package_root: &Path, command: &[String]) -> Result<(PathBuf, Vec<String>)> {
    let Some(program) = command.first() else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Plugin runtime command is empty",
        ));
    };
    let program_path = PathBuf::from(program);
    let resolved = if program_path.is_absolute() || program.contains(':') {
        program_path
    } else {
        package_root.join(program_path)
    };
    Ok((resolved, command.iter().skip(1).cloned().collect()))
}

/// Writes one newline-delimited JSON request to the plugin stdin stream.
fn write_json_line(
    writer: &mut BufWriter<ChildStdin>,
    value: &PluginRequestEnvelope,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|err| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Unable to encode plugin request: {err}"),
        )
    })?;
    bytes.push(b'\n');
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Reads one newline-delimited JSON response from the plugin stdout stream.
fn read_json_line<T: DeserializeOwned>(reader: &mut BufReader<ChildStdout>) -> Result<T> {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        return Err(Error::new(
            ErrorKind::UnexpectedEof,
            "Plugin worker closed stdout",
        ));
    }
    serde_json::from_str(line.trim_end()).map_err(|err| {
        Error::new(
            ErrorKind::InvalidData,
            format!("Unable to decode plugin response: {err}"),
        )
    })
}

/// Drains plugin stderr into a bounded in-memory buffer for later error context.
fn spawn_stderr_drain(stderr: ChildStderr, buffer: Arc<Mutex<VecDeque<String>>>) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let mut lines = buffer.lock().expect("stderr buffer lock poisoned");
            if lines.len() == 32 {
                lines.pop_front();
            }
            lines.push_back(line);
        }
    });
}
