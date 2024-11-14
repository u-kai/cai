use anyhow::Context;
use cai::{
    clients::{
        claude::ClaudeMessageClient, gemini::GeminiGenerateContent, openai::ChatCompletionsClient,
    },
    handlers::printer::Printer,
    tools::translator::{translate, TargetLang, TranslateRequests},
    AIError, Conversation, GenerativeAIInterface, Handler, MutHandler, Prompt,
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
        let prompt = if role_play.is_some() {
            Prompt::ask_with_role_play(question.as_str(), role_play.unwrap().as_str())
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
}

impl Into<Conversation> for ConversationInput {
    fn into(self) -> Conversation {
        let mut conversation = Conversation::new();
        for json in self.0 {
            match json.role {
                Role::AI => conversation.add_ai_message(json.comment.as_str()),
                Role::User => conversation.add_user_message(json.comment.as_str()),
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

macro_rules! gai_engine {
    ($($name:ident:$t:ty),*) => {
        enum GAIEngines {
            $(
                $name($t),
            )*
        }
        impl GAIEngines {
            async fn run_mut<H:MutHandler>(&self,handler:&mut H,prompt:Prompt)->Result<(),AIError> {
                match &self {
                    $(
                        &GAIEngines::$name(t) => t.request_mut(prompt,handler).await,
                    )*
                }

            }
        }
        impl GenerativeAIInterface for GAIEngines {
            async fn request<H:Handler>(&self,prompt:Prompt,handler:&H)->Result<(),AIError> {
                match &self {
                    $(
                        &GAIEngines::$name(t) => t.request(prompt,handler).await,
                    )*
                }
            }
            async fn request_mut<H:MutHandler>(&self,prompt:Prompt,handler:&mut H)->Result<(),AIError> {
                match &self {
                    $(
                        &GAIEngines::$name(t) => t.request_mut(prompt,handler).await,
                    )*
                }
            }
        }
    }
}

impl GAIEngines {
    fn from_str(engine: &str, key: String) -> Self {
        match engine {
            "gpt4" => GAIEngines::Gpt4(ChatCompletionsClient::gpt4(key)),
            "gpt4-o" => GAIEngines::Gpt4o(ChatCompletionsClient::gpt4o(key)),
            "gpt4-o-mini" => GAIEngines::Gpt4oMini(ChatCompletionsClient::gpt4o_mini(key)),
            "gpt3-5-turbo" => GAIEngines::Gpt3Dot5Turbo(ChatCompletionsClient::gpt3_5_turbo(key)),
            "gemini" => GAIEngines::Gemini15Flash(GeminiGenerateContent::new(key)),
            "claude3-haiku" => GAIEngines::Claude3Haiku(ClaudeMessageClient::haiku_3(key)),
            "claude3-ops" => GAIEngines::Claude3Ops(ClaudeMessageClient::ops_3(key)),
            "claude35-sonnet" => GAIEngines::Claude35Sonnet(ClaudeMessageClient::sonnet_3_5(key)),
            "claude3-sonnet" => GAIEngines::Claude3Sonnet(ClaudeMessageClient::sonnet_3(key)),
            _ => GAIEngines::Gpt4oMini(ChatCompletionsClient::gpt4o_mini(key)),
        }
    }
}

fn engine_to_default_key_from_env(engine: &str) -> String {
    if engine.contains("gpt") {
        return std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "".to_string());
    }
    if engine.contains("claude") {
        return std::env::var("CLAUDE_API_KEY").unwrap_or_else(|_| "".to_string());
    }
    if engine.contains("gemini") {
        return std::env::var("GEMINI_API_KEY").unwrap_or_else(|_| "".to_string());
    }
    panic!("Unknown engine: {}", engine);
}

gai_engine!(
    Gpt4:ChatCompletionsClient,
    Gpt4o:ChatCompletionsClient,
    Gpt4oMini:ChatCompletionsClient,
    Gpt3Dot5Turbo:ChatCompletionsClient,
    Gemini15Flash:GeminiGenerateContent,
    Claude3Haiku:ClaudeMessageClient,
    Claude3Ops:ClaudeMessageClient,
    Claude35Sonnet:ClaudeMessageClient,
    Claude3Sonnet:ClaudeMessageClient
);

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

            message = message.replace(&path, format!("{}", content).as_str());
            message
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{remove_file, File},
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
