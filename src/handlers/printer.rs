use std::io::Write;

use anyhow::Context;

use crate::{Handler, HandlerError, MutHandler};

pub struct Printer {}

impl Printer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Handler for Printer {
    async fn handle(&self, resp: &str) -> Result<(), HandlerError> {
        print!("{}", resp);
        Ok(std::io::stdout()
            .flush()
            .context("Failed to flush stdout")?)
    }
}
impl MutHandler for Printer {
    async fn handle_mut(&mut self, resp: &str) -> Result<(), HandlerError> {
        self.handle(resp).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{clients::openai::ChatCompletionsClient, Ask, GenerativeAIInterface, Prompt};

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_printer() {
        let printer = Printer::new();
        let chat = ChatCompletionsClient::gpt4o_mini(std::env::var("OPENAI_API_KEY").unwrap());
        let prompt = Prompt::Ask(Ask {
            question: "What is the meaning of life?".to_string(),
            role_play: None,
        });
        chat.request(prompt, &printer).await.unwrap();
    }
}
