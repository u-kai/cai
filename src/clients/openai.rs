use anyhow::Context;

use crate::AIError;
use crate::{sse::SseClient, GenerativeAIInterface, Prompt};

pub struct ChatCompletionsClient {
    inner: SseClient,
    api_key: String,
    model: ChatCompletionsModel,
}

impl ChatCompletionsClient {
    const URL: &'static str = "https://api.openai.com/v1/chat/completions";
    pub fn gpt4(api_key: String) -> Self {
        ChatCompletionsClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ChatCompletionsModel::Gpt4,
        }
    }
    pub fn gpt4o(api_key: String) -> Self {
        ChatCompletionsClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ChatCompletionsModel::Gpt4o,
        }
    }
    pub fn gpt4o_mini(api_key: String) -> Self {
        ChatCompletionsClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ChatCompletionsModel::Gpt4oMini,
        }
    }
    pub fn gpt3_5_turbo(api_key: String) -> Self {
        ChatCompletionsClient {
            inner: SseClient::new(Self::URL),
            api_key,
            model: ChatCompletionsModel::Gpt3Dot5Turbo,
        }
    }
    pub fn change_model(&mut self, model: ChatCompletionsModel) {
        self.model = model;
    }
}

impl GenerativeAIInterface for ChatCompletionsClient {
    async fn request<H: crate::Handler>(
        &self,
        prompt: crate::Prompt,
        handler: &H,
    ) -> Result<(), AIError> {
        let request = ChatRequest {
            model: self.model,
            messages: prompt.into(),
            stream: true,
        };

        let f = |stream: SseResponse| async {
            let data = match stream {
                SseResponse::Data(data) => data,
                _ => return Ok(()),
            };

            let resp = ChatResponse::try_from(data.as_str())
                .with_context(|| format!("Failed to parse response: {}", data.as_str()))
                .map_err(SseHandlerError::from)?;

            let resp = match resp {
                ChatResponse::Done => return Ok(()),
                ChatResponse::DeltaContent(content) => content,
            };

            handler
                .handle(resp.as_str())
                .await
                .with_context(|| format!("Failed to handle response: {}", resp.as_str()))
                .map_err(SseHandlerError::from)
        };
        self.inner
            .post()
            .bearer_auth(&self.api_key)
            .json(request)
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
    ) -> Result<(), AIError> {
        let request = ChatRequest {
            model: self.model,
            messages: prompt.into(),
            stream: true,
        };
        let f = |resp| {
            let data = match resp {
                SseResponse::Data(data) => data,
                _ => return Ok(String::new()),
            };
            let resp = ChatResponse::try_from(data.as_str())
                .with_context(|| format!("Failed to parse response: {}", data.as_str()))
                .map_err(SseHandleStreamError::from)?;
            let resp = match resp {
                ChatResponse::Done => return Ok(String::new()),
                ChatResponse::DeltaContent(content) => content,
            };
            Ok(resp)
        };

        self.inner
            .post()
            .bearer_auth(&self.api_key)
            .json(request)
            .request()
            .await
            .context("Failed to request")
            .map_err(AIError::from)?
            .handle_mut_stream_use_convert(f, handler)
            .await
            .with_context(|| "Failed to handle stream")
            .map_err(AIError::from)
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
struct ChatRequest {
    model: ChatCompletionsModel,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Debug, Copy, Clone, serde::Serialize, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ChatCompletionsModel {
    #[serde(rename = "gpt-3.5-turbo")]
    Gpt3Dot5Turbo,
    #[serde(rename = "gpt-4")]
    Gpt4,
    #[serde(rename = "gpt-4o-mini")]
    Gpt4oMini,
    #[serde(rename = "gpt-4o")]
    Gpt4o,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
struct Message {
    role: Role,
    content: String,
}

impl From<Prompt> for Vec<Message> {
    fn from(value: Prompt) -> Self {
        let messages = value.messages();
        messages.into_iter().map(Message::from).collect()
    }
}

impl From<crate::Message> for Message {
    fn from(value: crate::Message) -> Self {
        Self {
            role: value.role.into(),
            content: value.content,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    System,
    Assistant,
}
impl From<crate::Role> for Role {
    fn from(value: crate::Role) -> Self {
        match value {
            crate::Role::User => Self::User,
            crate::Role::AI => Self::System,
            crate::Role::RolePlay => Self::Assistant,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatResponse {
    Done,
    DeltaContent(String),
}
impl Default for ChatResponse {
    fn default() -> Self {
        Self::DeltaContent("".to_string())
    }
}
impl TryFrom<&str> for ChatResponse {
    type Error = serde_json::Error;
    fn try_from(stream: &str) -> Result<Self, Self::Error> {
        if stream.starts_with("[DONE]") {
            return Ok(Self::Done);
        }
        let resp = StreamChat::try_from(stream)?;
        Ok(resp.into())
    }
}

use crate::sse::{SseHandleStreamError, SseHandlerError, SseResponse};

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct StreamChat {
    choices: Vec<StreamChatChoices>,
    created: usize,
    id: String,
    model: String,
    object: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct StreamChatChoices {
    delta: StreamChatChoicesDelta,
    finish_reason: serde_json::Value,
    index: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamChatChoicesDelta {
    content: Option<String>,
}

impl TryFrom<&str> for StreamChat {
    type Error = serde_json::Error;
    fn try_from(stream: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(stream)
    }
}

impl From<StreamChat> for ChatResponse {
    fn from(s: StreamChat) -> Self {
        let mut s = s;
        s.choices.pop().map_or_else(
            || Self::default(),
            |c| {
                c.delta
                    .content
                    .map_or_else(|| Self::default(), Self::DeltaContent)
            },
        )
    }
}

#[cfg(test)]
mod tests {

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use crate::{clients::mocks::MockHandler, Conversation, Prompt};

    use super::*;
    #[tokio::test]
    #[ignore]
    async fn request_to_chatgpt() {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        let mut results = vec![];
        // I want to test the edge cases, so I will send a request three times.
        // To speed up, use asynchronous processing effectively and send requests asynchronously.
        for _ in 0..3 {
            let result = tokio::spawn(async {
                let mut handler = MockHandler::new();
                let sut = ChatCompletionsClient::gpt4o(std::env::var("OPENAI_API_KEY").unwrap());

                let mut conversation = Conversation::new();
                conversation.add_role_play_message("You are tom, who is a my friend");
                conversation.add_user_message("What your name?");

                let prompt = Prompt::with_conversation(conversation);

                sut.request_mut(prompt, &mut handler).await.unwrap();
                (handler.has_received, handler.received)
            });
            results.push(result);
        }
        for result in results {
            let (has_received, received) = result.await.unwrap();
            assert!(has_received);
            assert!(!received.is_empty());
            println!("Received: {}", received);
        }
    }
}