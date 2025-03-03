use anyhow::Context;
use cai::{
    AIError, Conversation, Prompt,
    clients::gai::{GAIEngines, engine_to_default_key_from_env},
    handlers::printer::Printer,
    server::AIServer,
    tools::translator::{TargetLang, TranslateRequests, translate},
};
use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.run().await {
        eprintln!("{}", e);
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    sub: SubCommand,
}
impl Cli {
    async fn run(&self) -> Result<(), AIError> {
        match &self.sub {
            SubCommand::Ask {
                question,
                engine,
                role_play,
            } => {
                self.ask(engine.to_string(), question.to_string(), role_play.clone())
                    .await
            }
            SubCommand::Translate {
                source,
                target_lang,
                engine,
                separate_per_limit,
            } => {
                self.translate(
                    engine.to_string(),
                    source.to_string(),
                    target_lang.to_string(),
                    *separate_per_limit,
                )
                .await
            }
            SubCommand::CodeReview { engine, path } => {
                self.code_review(engine.to_string(), path.to_string()).await
            }
            SubCommand::Conversation {
                engine,
                conversation,
            } => {
                self.conversation(engine.to_string(), conversation.to_string())
                    .await
            }
            SubCommand::Server { port } => self.server(*port).await,
        }
    }

    async fn conversation(&self, engine: String, conversation: String) -> Result<(), AIError> {
        let key = engine_to_default_key_from_env(engine.as_str());
        let ai = GAIEngines::from_str(&engine, key);

        let conversation: ConversationInput =
            serde_json::from_str(conversation.as_str()).context("Failed to parse conversation")?;

        let mut printer = Printer::new();

        let prompt = Prompt::Conversation(conversation.into());
        ai.run_mut(&mut printer, prompt).await?;

        Ok(())
    }
    async fn code_review(&self, engine: String, path: String) -> Result<(), AIError> {
        let key = engine_to_default_key_from_env(engine.as_str());
        let ai = GAIEngines::from_str(&engine, key);

        let file_contents =
            std::fs::read_to_string(path.as_str()).context("Failed to read file")?;

        let prompt = Prompt::ask(
            format!(
                "このファイルの内容をレビューしてください。\n{}",
                file_contents
            )
            .as_str(),
        );
        let mut printer = Printer::new();
        ai.run_mut(&mut printer, prompt).await
    }
    async fn translate(
        &self,
        engine: String,
        source: String,
        target_lang: String,
        separate_per_limit: usize,
    ) -> Result<(), AIError> {
        let key = engine_to_default_key_from_env(engine.as_str());
        let ai = GAIEngines::from_str(&engine, key);
        let separators = vec!['.', '!', '?'];
        if target_lang == "ja" {
            let request = TranslateRequests::new(source, TargetLang::Japanese)
                .separate_per_limit(separate_per_limit)
                .separators(separators);
            let response = translate(ai, request).await?;
            for res in response {
                println!("{}", res);
            }
        } else {
            let request = TranslateRequests::new(source, TargetLang::English)
                .separate_per_limit(separate_per_limit)
                .separators(separators);
            let response = translate(ai, request).await?;
            for res in response {
                println!("{}", res);
            }
        }
        Ok(())
    }
    async fn ask(
        &self,
        engine: String,
        question: String,
        role_play: Option<String>,
    ) -> Result<(), AIError> {
        let key = engine_to_default_key_from_env(engine.as_str());
        let ai = GAIEngines::from_str(&engine, key);
        let prompt = if let Some(role_play) = role_play {
            Prompt::ask_with_role_play(question.as_str(), role_play.as_str())
                .replace_messages(replace_remote_path_to_content)
                .replace_messages(replace_paths_to_content)
        } else {
            Prompt::ask(question.as_str())
                .replace_messages(replace_remote_path_to_content)
                .replace_messages(replace_paths_to_content)
        };
        let mut printer = Printer::new();
        ai.run_mut(&mut printer, prompt).await
    }
    async fn server(&self, port: u16) -> Result<(), AIError> {
        let server = AIServer::new(port);
        server.start().await;
        Ok(())
    }
}

#[derive(Subcommand)]
enum SubCommand {
    Ask {
        question: String,
        #[clap(long = "engine", short = 'e', default_value = "gpt4-o-mini")]
        engine: String,
        #[clap(short = 'r')]
        role_play: Option<String>,
    },
    #[clap(name = "conversation", alias = "conv")]
    Conversation {
        #[clap(long = "engine", short = 'e', default_value = "gpt4-o-mini")]
        engine: String,
        conversation: String,
    },
    #[clap(name = "code-review", alias = "cr")]
    CodeReview {
        #[clap(long = "engine", short = 'e', default_value = "gpt4-o-mini")]
        engine: String,
        path: String,
    },
    #[clap(name = "translate", alias = "t")]
    Translate {
        source: String,
        #[clap(long = "target-lang", short = 't', default_value = "ja")]
        target_lang: String,
        #[clap(long = "engine", short = 'e', default_value = "gpt4-o-mini")]
        engine: String,
        #[clap(short = 'l', default_value = "1")]
        separate_per_limit: usize,
    },
    #[clap(name = "server")]
    Server {
        #[clap(long = "port", short = 'p', default_value = "9999")]
        port: u16,
    },
}

impl From<ConversationInput> for Conversation {
    fn from(input: ConversationInput) -> Conversation {
        let mut conversation = Conversation::new();
        for message in input.0 {
            match message.role {
                Role::AI => conversation.add_ai_message(&message.comment),
                Role::User => conversation.add_user_message(&message.comment),
            }
        }
        conversation
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ConversationInput(Vec<ConversationJson>);

#[derive(Debug, Clone, serde::Deserialize)]
struct ConversationJson {
    role: Role,
    comment: String,
}
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    AI,
    User,
}

fn replace_paths_to_content(message: String) -> String {
    let Ok(re) = regex::Regex::new(r"\{([^}]+)\}") else {
        return message.to_string();
    };
    re.captures_iter(message.as_str())
        .fold(message.to_string(), |mut message, cap| {
            let Some(path) = cap.get(1) else {
                return message;
            };
            let Ok(content) = std::fs::read_to_string(path.as_str()) else {
                return message;
            };
            let path = format!("{{{}}}", path.as_str());

            message = message.replace(&path, format!("```{}```", content).as_str());
            message
        })
}

fn replace_remote_path_to_content(message: String) -> String {
    let Ok(re) = regex::Regex::new(r"\[([^}]+)\]") else {
        return message.to_string();
    };
    re.captures_iter(message.as_str())
        .fold(message.to_string(), |mut message, cap| {
            let Some(path) = cap.get(1) else {
                return message;
            };
            let Ok(content) = reqwest::blocking::get(path.as_str()) else {
                return message;
            };
            let content = content.text().unwrap_or_else(|_| "".to_string());
            let path = format!("[{}]", path.as_str());

            message = message.replace(&path, content.as_str());
            message
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{File, remove_file},
        io::Write,
    };

    use super::*;
    #[test]
    fn replace_content() {
        let mut f = File::create("test.txt").unwrap();
        f.write_all(b"test").unwrap();
        let mut f = File::create("test2.txt").unwrap();
        f.write_all(b"test2").unwrap();

        let message = "review following code, {test.txt} and {test2.txt}";

        let sut = replace_paths_to_content(message.to_string());

        remove_file("test.txt").unwrap();
        remove_file("test2.txt").unwrap();

        assert_eq!(sut, "review following code, ```test``` and ```test2```");
    }
}
