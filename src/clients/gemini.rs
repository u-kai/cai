use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{
    sse::{SseClient, SseResponse},
    AIError, GenerativeAIInterface, Handler, MutHandler, Prompt, Role,
};

pub struct GeminiGenerateContent {
    inner: SseClient,
    api_key: String,
}
impl GeminiGenerateContent {
    const BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta/models/";
    pub fn new(api_key: String) -> Self {
        let model = GeminiModel::Gemini15Flash;
        let url = format!("{}{}:streamGenerateContent", Self::BASE_URL, model.to_str());
        GeminiGenerateContent {
            inner: SseClient::new(url.as_str()),
            api_key,
        }
    }
}

impl GenerativeAIInterface for GeminiGenerateContent {
    async fn request<H: Handler>(&self, prompt: Prompt, handler: &H) -> Result<(), AIError> {
        let f = |stream: crate::sse::SseResponse| async {
            let data = match stream {
                crate::sse::SseResponse::Data(data) => data,
                _ => return Ok(()),
            };

            let resp = serde_json::from_str::<GeminiResponse>(data.as_str())
                .with_context(|| format!("Failed to parse response: {}", data.as_str()))?;

            let content: String = resp.into();

            Ok(handler
                .handle(content.as_str())
                .await
                .context("Failed to handle response")?)
        };

        Ok(self
            .inner
            .post()
            .query(&[("key", self.api_key.as_str()), ("alt", "sse")])
            .json(&GeminiRequest::from(prompt))
            .request()
            .await
            .context("Failed to request")?
            .handle_stream(&f)
            .await
            .with_context(|| "Failed to handle stream")?)
    }

    async fn request_mut<H: MutHandler>(
        &self,
        prompt: Prompt,
        handler: &mut H,
    ) -> Result<(), AIError> {
        let f = |stream| {
            let data = match stream {
                SseResponse::Data(data) => data,
                _ => return Ok(String::new()),
            };

            let resp = serde_json::from_str::<GeminiResponse>(data.as_str())
                .with_context(|| format!("Failed to parse response: {}", data.as_str()))?;

            let content: String = resp.into();
            Ok(content)
        };

        Ok(self
            .inner
            .post()
            .query(&[("key", self.api_key.as_str()), ("alt", "sse")])
            .json(&GeminiRequest::from(prompt))
            .request()
            .await
            .context("Failed to request")?
            .handle_mut_stream_use_convert(f, handler)
            .await
            .with_context(|| "Failed to handle stream")?)
    }
}

pub enum GeminiModel {
    Gemini15Flash,
}
impl GeminiModel {
    pub fn to_str(&self) -> &'static str {
        match self {
            GeminiModel::Gemini15Flash => "gemini-1.5-flash",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiRequest {
    contents: Vec<GeminiContent>,
}
impl From<Prompt> for GeminiRequest {
    fn from(prompt: Prompt) -> Self {
        match prompt {
            Prompt::Ask(ask) => {
                if let Some(role_play) = ask.role_play {
                    let role_play = GeminiContent {
                        parts: vec![GeminiContentPart { text: role_play }],
                        role: GeminiRole::User,
                    };
                    GeminiRequest {
                        contents: vec![
                            role_play,
                            GeminiContent {
                                parts: vec![GeminiContentPart { text: ask.question }],
                                role: GeminiRole::User,
                            },
                        ],
                    }
                } else {
                    GeminiRequest {
                        contents: vec![GeminiContent {
                            parts: vec![GeminiContentPart { text: ask.question }],
                            role: GeminiRole::User,
                        }],
                    }
                }
            }
            Prompt::Conversation(conversation) => GeminiRequest {
                contents: conversation
                    .messages
                    .into_iter()
                    .map(|message| GeminiContent {
                        parts: vec![GeminiContentPart {
                            text: message.content,
                        }],
                        role: message.role.into(),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    parts: Vec<GeminiContentPart>,
    role: GeminiRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContentPart {
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeminiRole {
    User,
    Model,
}

impl From<Role> for GeminiRole {
    fn from(role: Role) -> Self {
        match role {
            Role::User => GeminiRole::User,
            Role::AI => GeminiRole::Model,
            Role::RolePlay => GeminiRole::User,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeminiResponse {
    candidates: Vec<GeminiResponseCandidate>,
}
impl From<GeminiResponse> for String {
    fn from(response: GeminiResponse) -> String {
        response
            .candidates
            .into_iter()
            .next()
            .map(|c| c.into())
            .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct GeminiResponseCandidate {
    content: GeminiContent,
}

impl From<GeminiResponseCandidate> for String {
    fn from(candidate: GeminiResponseCandidate) -> Self {
        candidate
            .content
            .parts
            .into_iter()
            .next()
            .map(|p| p.text)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use crate::{clients::mocks::MockHandler, Conversation, Prompt};

    use super::*;
    #[ignore]
    #[tokio::test]
    async fn request_to_gemini() {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
        let client = GeminiGenerateContent::new(std::env::var("GEMINI_API_KEY").unwrap());
        let prompt = Prompt::ask("What is the meaning of life?");
        let handler = MockHandler::new();
        client.request(prompt, &handler).await.unwrap();

        let mut conversation = Conversation::new();
        conversation.add_role_play_message("You are tom, who is a my friend");
        conversation.add_user_message("What your name?");
        let prompt = Prompt::with_conversation(conversation);
        let mut handler = MockHandler::new();

        client.request_mut(prompt, &mut handler).await.unwrap();
        println!("Received: {}", handler.received);
        assert!(handler.has_received);
    }
}
