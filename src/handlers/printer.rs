use std::{
    cell::RefCell,
    fs::File,
    io::{Stdout, Write},
    path::Path,
};

use anyhow::Context;

use crate::{Handler, HandlerError, MutHandler};

pub struct Printer<W: Write> {
    writer: RefCell<W>,
}

impl Printer<Stdout> {
    pub fn new() -> Self {
        Self {
            writer: RefCell::new(std::io::stdout()),
        }
    }
}
impl Printer<File> {
    pub fn new_file(path: impl AsRef<Path>) -> Self {
        let file = File::create(path).expect("Failed to create file");
        Self {
            writer: RefCell::new(file),
        }
    }
    pub fn open_file(path: impl AsRef<Path>) -> Self {
        let file = File::open(path).expect("Failed to open file");
        Self {
            writer: RefCell::new(file),
        }
    }
}

impl<W: Write> Handler for Printer<W> {
    async fn handle(&self, resp: &str) -> Result<(), HandlerError> {
        // こっちの方が滑らか
        print!("{}", resp);
        std::io::stdout()
            .flush()
            .context("Failed to flush stdout")
            .map_err(HandlerError)
        // self.writer
        //     .borrow_mut()
        //     .write_all(resp.as_bytes())
        //     .context("Failed to write to stdout")
        //     .map_err(HandlerError)
    }
}
impl<W: Write> MutHandler for Printer<W> {
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
