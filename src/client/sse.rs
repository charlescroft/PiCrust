use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum AgentSseEvent {
    #[serde(rename = "connected")]
    Connected {
        #[serde(default, rename = "isStreaming")]
        is_streaming: bool,
    },
    #[serde(rename = "agent_start")]
    AgentStart,
    #[serde(rename = "agent_end")]
    AgentEnd {
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "prompt_done")]
    PromptDone,
    #[serde(rename = "agent_settled")]
    AgentSettled,
    #[serde(rename = "turn_start")]
    TurnStart,
    #[serde(rename = "turn_end")]
    TurnEnd {
        #[serde(default)]
        duration_ms: Option<u64>,
        #[serde(default)]
        usage: Option<Value>,
    },
    #[serde(rename = "message_start")]
    MessageStart {
        #[serde(default)]
        message: Option<Value>,
    },
    #[serde(rename = "message_update")]
    MessageUpdate {
        #[serde(default, rename = "assistantMessageEvent")]
        assistant_message_event: Option<Value>,
        #[serde(default)]
        delta: Option<String>,
        #[serde(default)]
        thinking_delta: Option<String>,
    },
    #[serde(rename = "message_end")]
    MessageEnd {
        #[serde(default)]
        message: Option<Value>,
    },
    #[serde(rename = "tool_execution_start")]
    ToolExecutionStart {
        #[serde(default, rename = "toolCallId")]
        tool_call_id: String,
        #[serde(default, rename = "toolName")]
        tool_name: String,
        #[serde(default)]
        args: Option<Value>,
    },
    #[serde(rename = "tool_execution_update")]
    ToolExecutionUpdate {
        #[serde(default, rename = "toolCallId")]
        tool_call_id: String,
        #[serde(default, rename = "partialResult")]
        partial_result: Option<Value>,
    },
    #[serde(rename = "tool_execution_end")]
    ToolExecutionEnd {
        #[serde(default, rename = "toolCallId")]
        tool_call_id: String,
        #[serde(default, rename = "toolName")]
        tool_name: String,
        #[serde(default)]
        result: Option<Value>,
        #[serde(default, rename = "isError")]
        is_error: bool,
    },
    #[serde(rename = "extension_ui_request")]
    ExtensionUiRequest {
        #[serde(flatten)]
        payload: Value,
    },
    #[serde(rename = "compaction_start")]
    CompactionStart,
    #[serde(rename = "compaction_end")]
    CompactionEnd {
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        result: Option<Value>,
        #[serde(default)]
        aborted: Option<bool>,
        #[serde(default, rename = "errorMessage")]
        error_message: Option<String>,
    },
    #[serde(rename = "auto_retry_start")]
    AutoRetryStart {
        #[serde(default)]
        attempt: Option<u32>,
        #[serde(default, rename = "maxAttempts")]
        max_attempts: Option<u32>,
        #[serde(default, rename = "errorMessage")]
        error_message: Option<String>,
    },
    #[serde(rename = "auto_retry_end")]
    AutoRetryEnd {
        #[serde(default)]
        success: Option<bool>,
        #[serde(default, rename = "finalError")]
        final_error: Option<String>,
        #[serde(default, rename = "errorMessage")]
        error_message: Option<String>,
    },
    #[serde(rename = "prompt_error")]
    PromptError {
        #[serde(default, rename = "errorMessage")]
        error_message: Option<String>,
    },
    #[serde(rename = "extension_error")]
    ExtensionError {
        #[serde(default)]
        error: Option<String>,
    },
    #[serde(rename = "startup_error")]
    StartupError {
        #[serde(default, rename = "errorMessage")]
        error_message: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

pub async fn consume_agent_sse<F>(
    client: Client,
    url: String,
    token: String,
    instance_id: String,
    mut on_event: F,
) -> anyhow::Result<()>
where
    F: FnMut(AgentSseEvent) -> bool,
{
    let (tx, rx) = smol::channel::unbounded::<Result<AgentSseEvent, String>>();

    crate::client::tokio_runtime().spawn(async move {
        let resp = match client
            .get(&url)
            .header("x-pi-desktop-token", &token)
            .header("x-pi-desktop-instance", &instance_id)
            .header("accept", "text/event-stream")
            .timeout(Duration::from_secs(3600))
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Err(e.to_string())).await;
                return;
            }
        };

        let mut stream = resp.bytes_stream().eventsource();
        while let Some(item) = stream.next().await {
            match item {
                Ok(ev) => {
                    if ev.data.trim().is_empty() || ev.data == "[DONE]" {
                        continue;
                    }
                    if let Ok(parsed) = serde_json::from_str::<AgentSseEvent>(&ev.data) {
                        if tx.send(Ok(parsed)).await.is_err() {
                            break;
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(format!("SSE stream error: {err}"))).await;
                    break;
                }
            }
        }
    });

    while let Ok(msg) = rx.recv().await {
        match msg {
            Ok(ev) => {
                if !on_event(ev) {
                    break;
                }
            }
            Err(err) => return Err(anyhow::anyhow!(err)),
        }
    }
    Ok(())
}

pub async fn consume_json_sse<F>(
    client: Client,
    url: String,
    token: String,
    instance_id: String,
    mut on_payload: F,
) -> anyhow::Result<()>
where
    F: FnMut(Value) -> bool,
{
    let (tx, rx) = smol::channel::unbounded::<Value>();

    crate::client::tokio_runtime().spawn(async move {
        let Ok(resp) = client
            .get(&url)
            .header("x-pi-desktop-token", &token)
            .header("x-pi-desktop-instance", &instance_id)
            .header("accept", "text/event-stream")
            .timeout(Duration::from_secs(3600))
            .send()
            .await
            .and_then(|r| r.error_for_status())
        else {
            return;
        };

        let mut stream = resp.bytes_stream().eventsource();
        while let Some(item) = stream.next().await {
            if let Ok(ev) = item {
                if ev.data.trim().is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<Value>(&ev.data) {
                    if tx.send(val).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Ok(val) = rx.recv().await {
        if !on_payload(val) {
            break;
        }
    }
    Ok(())
}
