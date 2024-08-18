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
    pub fn replace_messages<F>(self, f: F) -> Self
    where
        F: Fn(String) -> String,
    {
        match self {
            Prompt::Ask(ask) => Self::Ask(Ask {
                question: f(ask.question),
                role_play: ask.role_play,
            }),
            Prompt::Conversation(conversation) => {
                let mut new_conversation = Conversation::new();
                for message in conversation.messages {
                    match message.role {
                        Role::AI => new_conversation.add_ai_message(&f(message.content)),
                        Role::User => new_conversation.add_user_message(&f(message.content)),
                        Role::RolePlay => {
                            new_conversation.add_role_play_message(&f(message.content))
                        }
                    }
                }
                Self::Conversation(new_conversation)
            }
        }
    }
    const SPLIT_CHARACTERS: [char; 6] = ['.', '!', '?', '。', '！', '？'];
    // Split a large message by maximum length
    // base_message: message to prepend
    // message: Message to be split
    // max_length: Maximum length of the message
    // Basically, this is used when the instruction content, such as a translation request, can be interrupted without any issues.
    // It's preferable not to use this function when there is only one message (e.g., for code reviews).
    // If the context of the premise is important, include it in the base_message.
    pub fn split_by_max_length(base_message: &str, message: &str, max_length: usize) -> Vec<Self> {
        message
            .split(|c| Self::SPLIT_CHARACTERS.contains(&c))
            .fold(vec![], |mut acc, sentence| {
                if sentence.is_empty() {
                    return acc;
                }
                if acc.is_empty() {
                    acc.push(Ask {
                        question: format!("{}{}.", base_message, sentence),
                        role_play: None,
                    })
                } else {
                    let last = acc.last_mut().unwrap();
                    // 1 is for the period.
                    if last.question.len() + sentence.len() + 1 <= max_length {
                        last.question.push_str(&format!("{}.", sentence));
                    } else {
                        acc.push(Ask {
                            question: format!("{}{}.", base_message, sentence),
                            role_play: None,
                        });
                    }
                }
                acc
            })
            .into_iter()
            .map(|ask| Self::Ask(ask))
            .collect()
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
    fn prompt_can_replace_by_function() {
        let prompt = Prompt::ask("What is the meaning of life?");
        let prompt = prompt.replace_messages(|q| q.to_uppercase());
        match prompt {
            Prompt::Ask(ask) => {
                assert_eq!(ask.question, "WHAT IS THE MEANING OF LIFE?");
                assert_eq!(ask.role_play, None);
            }
            _ => panic!("Unexpected prompt type"),
        }
    }
    #[test]
    fn split_by_max_length_should_split_by_max_length_and_period() {
        let base_message = "Please translate next sentence: ";
        let big_message_src = "I was foo and bar.";
        let big_message = big_message_src.repeat(100);
        // This value is less than or equal to the total number of characters in base_message and big_message
        // and also less than or equal to twice the total number of characters.
        // By doing this, the concatenated value of base_message and big_message becomes a single prompt.
        let max_length = 60;

        let prompts = Prompt::split_by_max_length(base_message, &big_message, max_length);

        assert_eq!(prompts.len(), 100);
        prompts.iter().for_each(|prompt| match prompt {
            Prompt::Ask(ask) => {
                assert_eq!(ask.question, format!("{}{}", base_message, big_message_src));
                assert_eq!(ask.role_play, None);
            }
            _ => panic!("Unexpected prompt type"),
        });
    }
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
