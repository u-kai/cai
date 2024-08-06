use anyhow::Context;

use crate::{sse::SseClient, AIError, GenerativeAIInterface, Prompt};

pub struct ClaudeMessageClient {
    inner: SseClient,
    api_key: String,
    model: ClaudeModel,
}

impl ClaudeMessageClient {
    const URL: &'static str = "https://api.anthropic.com/v1/messages";
    pub fn sonnet_3_5(api_key: String) -> Self {
        ClaudeMessageClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ClaudeModel::Claude35Sonnet,
        }
    }
    pub fn sonnet_3(api_key: String) -> Self {
        ClaudeMessageClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ClaudeModel::Claude3Sonnet,
        }
    }
    pub fn ops_3(api_key: String) -> Self {
        ClaudeMessageClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ClaudeModel::Claude3Ops,
        }
    }
    pub fn haiku_3(api_key: String) -> Self {
        ClaudeMessageClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ClaudeModel::Claude3Haiku,
        }
    }
}
impl GenerativeAIInterface for ClaudeMessageClient {
    async fn request<H: crate::Handler>(
        &self,
        prompt: crate::Prompt,
        handler: &H,
    ) -> Result<(), crate::AIError> {
        let f = |stream: crate::sse::SseResponse| async {
            let data = match stream {
                crate::sse::SseResponse::Data(data) => data,
                _ => return Ok(()),
            };

            let Ok(resp) = serde_json::from_str::<ClaudeMessageStreamResponse>(data.as_str())
            else {
                return Ok(());
            };

            handler
                .handle(resp.into_string().as_str())
                .await
                .context("Failed to handle response")
                .map_err(crate::sse::SseHandlerError::from)
        };

        self.inner
            .post()
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", self.api_key.as_str())
            .json(&ClaudeMessageRequest::new(self.model, prompt))
            .request()
            .await
            .context("Failed to request")
            .map_err(AIError)?
            .handle_stream(&f)
            .await
            .with_context(|| "Failed to handle stream")
            .map_err(AIError)
    }
    async fn request_mut<H: crate::MutHandler>(
        &self,
        prompt: crate::Prompt,
        handler: &mut H,
    ) -> Result<(), crate::AIError> {
        let f = |stream| {
            let data = match stream {
                crate::sse::SseResponse::Data(data) => data,
                _ => return Ok(String::new()),
            };
            let Ok(resp) = serde_json::from_str::<ClaudeMessageStreamResponse>(data.as_str())
            else {
                return Ok(String::new());
            };

            Ok(resp.into_string())
        };

        self.inner
            .post()
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", self.api_key.as_str())
            .json(&ClaudeMessageRequest::new(self.model, prompt))
            .request()
            .await
            .context("Failed to request")
            .map_err(AIError)?
            .handle_mut_stream_use_convert(f, handler)
            .await
            .with_context(|| "Failed to handle stream")
            .map_err(AIError)
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ClaudeMessageRequest {
    max_tokens: usize,
    messages: Vec<ClaudeMessageRequestMessages>,
    model: ClaudeModel,
    stream: bool,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ClaudeMessageRequestMessages {
    content: String,
    role: String,
}
impl ClaudeMessageRequest {
    fn new(model: ClaudeModel, prompt: Prompt) -> Self {
        ClaudeMessageRequest {
            max_tokens: 1024,
            messages: prompt.into(),
            model,
            stream: true,
        }
    }
}

impl From<Prompt> for Vec<ClaudeMessageRequestMessages> {
    fn from(prompt: Prompt) -> Self {
        match prompt {
            Prompt::Ask(ask) => {
                if let Some(role_play) = ask.role_play {
                    let role_play = ClaudeMessageRequestMessages {
                        content: role_play,
                        role: "user".to_string(),
                    };
                    vec![
                        role_play,
                        ClaudeMessageRequestMessages {
                            content: ask.question,
                            role: "user".to_string(),
                        },
                    ]
                } else {
                    vec![ClaudeMessageRequestMessages {
                        content: ask.question,
                        role: "user".to_string(),
                    }]
                }
            }
            Prompt::Conversation(conversation) => conversation
                .messages()
                .into_iter()
                .map(|m| {
                    let role = if m.role == crate::Role::User {
                        "user"
                    } else {
                        "assistant"
                    };
                    ClaudeMessageRequestMessages {
                        content: m.content,
                        role: role.to_string(),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Copy)]
enum ClaudeModel {
    Claude35Sonnet,
    Claude3Ops,
    Claude3Sonnet,
    Claude3Haiku,
}
impl ClaudeModel {
    fn to_str(&self) -> &'static str {
        match self {
            ClaudeModel::Claude35Sonnet => "claude-3-5-sonnet-20240620",
            ClaudeModel::Claude3Ops => "claude-3-opus-20240229",
            ClaudeModel::Claude3Sonnet => "claude-3-sonnet-20240229",
            ClaudeModel::Claude3Haiku => "claude-3-haiku-20240307",
        }
    }
}
impl serde::Serialize for ClaudeModel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_str())
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ClaudeMessageStreamResponse {
    pub delta: ClaudeMessageStreamResponseDelta,
    pub index: usize,
    #[serde(rename = "type")]
    pub r#type: String,
}
impl ClaudeMessageStreamResponse {
    fn into_string(self) -> String {
        self.delta.text
    }
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ClaudeMessageStreamResponseDelta {
    pub text: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[cfg(test)]
mod tests {
    use crate::{clients::mocks::MockHandler, Prompt};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use super::*;

    #[ignore]
    #[tokio::test]
    async fn request_to_claude() {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
        let client = ClaudeMessageClient::sonnet_3_5(std::env::var("CLAUDE_API_KEY").unwrap());
        let prompt = Prompt::ask("What is the meaning of life?");
        let mut handler = MockHandler::new();

        client.request_mut(prompt, &mut handler).await.unwrap();

        println!("Received{:?}", handler.received);
        assert!(handler.has_received);
    }
}
