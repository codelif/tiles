//! Module that deals with Pi
use crate::core::agent::types::{GetStateData, PiResponse};
use crate::utils::config::{
    ConfigProvider, DefaultProvider, create_pi_provider_config, handle_pi_settings_config,
};
use anyhow::{Context, Result, anyhow};
use nix::unistd::setsid;
use serde_json::{Value, json};
use std::{fs, process::Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct PiAgent {
    pub process: Child,
    pub writer: PiWriter,
    pub reader: PiReader,
}

pub struct PiWriter {
    stdin: ChildStdin,
}

pub struct PiReader {
    lines: Lines<BufReader<ChildStdout>>,
}

pub fn new(model_name: &str, system_prompt: &str, port: u32) -> Result<PiAgent> {
    let tiles_lib_dir = DefaultProvider.get_lib_dir()?;
    let user_data_dir = DefaultProvider.get_user_data_dir()?;
    let pi_agent_dir = user_data_dir.join("pi/agent/");
    std::fs::create_dir_all(&pi_agent_dir).context("Failed to create Pi agent directory")?;

    let provider_config_file_path = pi_agent_dir.join("models.json");
    let endpoint_url = format!("http://127.0.0.1:{}/v1", port);
    let model_config = create_pi_provider_config(model_name, &endpoint_url)?;

    fs::write(provider_config_file_path, model_config)?;

    let settings_file_path = pi_agent_dir.join("settings.json");
    handle_pi_settings_config(&settings_file_path)?;

    let pi_exec_path = tiles_lib_dir.join("pi/pi");

    let mut pi_process = unsafe {
        Command::new(pi_exec_path)
            .arg("--mode")
            .arg("rpc")
            .arg("--append-system-prompt")
            .arg(system_prompt)
            .arg("--no-session")
            .env("PI_CODING_AGENT_DIR", pi_agent_dir)
            .env("PI_OFFLINE", "true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .pre_exec(|| {
                if let Err(err) = setsid() {
                    Err(Into::into(err))
                } else {
                    Ok(())
                }
            })
            .spawn()
            .expect("failed to run Pi")
    };

    //TODO: Remove the unwrap
    let pi_stdin = pi_process.stdin.take().unwrap();
    let pi_stdout = pi_process.stdout.take().expect("stdout");

    Ok(PiAgent {
        process: pi_process,
        reader: PiReader {
            lines: BufReader::new(pi_stdout).lines(),
        },
        writer: PiWriter { stdin: pi_stdin },
    })
}
pub async fn handle_graceful_exit(writer: &mut PiWriter) -> Result<()> {
    let end_payload = json!({
        "type": "abort",
    });
    writer.send_to_pi(end_payload).await
}

//TODO: we should make the member function names pi agnostic..
impl PiAgent {
    pub fn split(self) -> (Child, PiReader, PiWriter) {
        (self.process, self.reader, self.writer)
    }
}

//TODO: move this to an associative function

impl PiWriter {
    pub async fn send_to_pi(&mut self, payload_json: Value) -> Result<()> {
        let payload_str = format!("{}\n", serde_json::to_string(&payload_json)?);
        self.stdin
            .write_all(payload_str.as_bytes())
            .await
            .context("Failed to send to Pi's stdin")?;
        self.stdin
            .flush()
            .await
            .context("Failed to flush Pi stdin")?;
        Ok(())
    }
}

impl PiReader {
    pub async fn get_pi_state(&mut self, writer: &mut PiWriter) -> Result<GetStateData> {
        let init_cmd_payload = json!({
            "type": "get_state",
        });

        writer
            .send_to_pi(init_cmd_payload)
            .await
            .inspect_err(|_e| eprintln!("sending command to  pi failed"))?;

        if let Some(line) = self.lines.next_line().await? {
            let response: PiResponse = serde_json::from_str(&line)?;
            if let PiResponse::Response(msg) = response {
                let state: GetStateData =
                    serde_json::from_value(msg.data.expect("get state parsing failed"))?;
                Ok(state)
            } else {
                Err(anyhow!("Failed to fetch initial state from Pi"))
            }
        } else {
            Err(anyhow!("Failed to fetch session_id from Pi"))
        }
    }

    pub async fn next_line(&mut self) -> std::result::Result<Option<String>, std::io::Error> {
        self.lines.next_line().await
    }
}
