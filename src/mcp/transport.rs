//! Stdio transport layer for MCP server communication.
//!
//! Spawns MCP servers as subprocesses and communicates via newline-delimited JSON.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Stdio transport for communicating with an MCP server subprocess
pub struct StdioTransport {
    pub child: Child,
    pub stdin: ChildStdin,
    pub response_rx: Receiver<Value>,
    reader_handle: Option<JoinHandle<()>>,
}

impl StdioTransport {
    /// Spawn an MCP server subprocess and set up communication channels
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: &Path,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(cwd)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Let server errors show in terminal

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {}", command))?;

        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = child.stdout.take().expect("Failed to get stdout");

        let (tx, rx) = mpsc::channel();

        // Spawn reader thread to process stdout
        let reader_handle = thread::spawn(move || {
            Self::reader_loop(stdout, tx);
        });

        Ok(Self {
            child,
            stdin,
            response_rx: rx,
            reader_handle: Some(reader_handle),
        })
    }

    /// Reader loop that processes newline-delimited JSON from stdout
    fn reader_loop(stdout: ChildStdout, tx: Sender<Value>) {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) if !line.is_empty() => {
                    match serde_json::from_str(&line) {
                        Ok(msg) => {
                            if tx.send(msg).is_err() {
                                // Receiver dropped, exit loop
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("MCP: Failed to parse JSON from server: {}", e);
                            eprintln!("MCP: Line was: {}", line);
                        }
                    }
                }
                Err(_) => break, // Pipe closed
                _ => {}
            }
        }
    }

    /// Send a JSON-RPC message to the MCP server
    pub fn send(&mut self, message: &Value) -> Result<()> {
        let json = serde_json::to_string(message)?;
        writeln!(self.stdin, "{}", json).context("Failed to write to MCP server stdin")?;
        self.stdin
            .flush()
            .context("Failed to flush MCP server stdin")?;
        Ok(())
    }

    /// Receive a response with timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Value> {
        self.response_rx
            .recv_timeout(timeout)
            .map_err(|e| anyhow::anyhow!("Receive timeout: {}", e))
    }

    /// Check if the server process is still alive
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // Process exited
            Ok(None) => true,     // Still running
            Err(_) => false,      // Error checking status
        }
    }

    /// Get the process ID of the child
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Get exit status if the process has exited
    pub fn exit_status(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        }
    }

    /// Kill the server process
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("Failed to kill MCP server")?;
        self.child.wait().context("Failed to wait for MCP server")?;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Attempt to kill the child process if still running
        let _ = self.child.kill();
        let _ = self.child.wait();

        // Wait for reader thread to finish
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}
