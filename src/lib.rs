pub mod clients;
pub mod handlers;
pub mod sse;

pub trait GenerativeAIInterface {
    #[allow(async_fn_in_trait)]
    async fn request<H: Handler>(&self, prompt: Prompt, handler: &H) -> Result<(), AIError>;
    #[allow(async_fn_in_trait)]
    async fn request_mut<H: MutHandler>(
        &self,
        prompt: Prompt,
        handler: &mut H,
    ) -> Result<(), AIError>;
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct AIError(anyhow::Error);

#[macro_export]
macro_rules! impl_from_error {
    ($($error:ty),*) => {
        $(
            impl From<anyhow::Error> for $error {
                fn from(e:anyhow::Error) -> Self {
                    Self(e)
                }
            }
        )*
    };
}

impl_from_error!(AIError, HandlerError);

pub trait Handler {
    #[allow(async_fn_in_trait)]
    async fn handle(&self, resp: &str) -> Result<(), HandlerError>;
}

pub trait MutHandler {
    #[allow(async_fn_in_trait)]
    async fn handle_mut(&mut self, resp: &str) -> Result<(), HandlerError>;
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct HandlerError(anyhow::Error);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    Ask(Ask),
    Conversation(Conversation),
}

impl Prompt {
    pub fn ask(question: &str) -> Self {
        Self::Ask(Ask {
            question: question.to_string(),
            role_play: None,
        })
    }
    pub fn ask_with_role_play(question: &str, role_play: &str) -> Self {
        Self::Ask(Ask {
            question: question.to_string(),
            role_play: Some(role_play.to_string()),
        })
    }
    pub fn with_conversation(conversation: Conversation) -> Self {
        Self::Conversation(conversation)
    }
    pub fn messages(self) -> Vec<Message> {
        match self {
            Prompt::Ask(ask) => ask.messages(),
            Prompt::Conversation(conversation) => conversation.messages(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    role: Role,
    content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    AI,
    RolePlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    question: String,
    role_play: Option<String>,
}
impl Ask {
    fn messages(self) -> Vec<Message> {
        match self.role_play {
            Some(role_play) => vec![
                Message {
                    role: Role::RolePlay,
                    content: role_play,
                },
                Message {
                    role: Role::User,
                    content: self.question,
                },
            ],
            None => vec![Message {
                role: Role::User,
                content: self.question,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add_role_play_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::RolePlay,
            content: content.to_string(),
        });
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::User,
            content: content.to_string(),
        });
    }

    pub fn add_ai_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::AI,
            content: content.to_string(),
        });
    }

    pub fn messages(self) -> Vec<Message> {
        self.messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation() {
        let mut conversation = Conversation::new();
        conversation.add_role_play_message("You are a teacher.");
        conversation.add_user_message("What is the meaning of life?");
        conversation.add_ai_message("The meaning of life is 42.");

        let messages = conversation.messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::RolePlay);
        assert_eq!(messages[0].content, "You are a teacher.");
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(messages[1].content, "What is the meaning of life?");
        assert_eq!(messages[2].role, Role::AI);
        assert_eq!(messages[2].content, "The meaning of life is 42.");
    }
}
