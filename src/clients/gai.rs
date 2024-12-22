use super::{
    claude::ClaudeMessageClient, gemini::GeminiGenerateContent, openai::ChatCompletionsClient,
};
use crate::{AIError, GenerativeAIInterface, Handler, MutHandler, Prompt};

macro_rules! gai_engine {
    ($($name:ident:$t:ty),*) => {
        pub enum GAIEngines {
            $(
                $name($t),
            )*
        }
        impl GAIEngines {
            pub async fn run_mut<H:MutHandler>(&self,handler:&mut H,prompt:Prompt)->Result<(),AIError> {
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
    pub fn from_str(engine: &str, key: String) -> Self {
        match engine {
            "gpt4" => GAIEngines::Gpt4(ChatCompletionsClient::gpt4(key)),
            "gpt4-o" => GAIEngines::Gpt4o(ChatCompletionsClient::gpt4o(key)),
            "gpt4-o-mini" => GAIEngines::Gpt4oMini(ChatCompletionsClient::gpt4o_mini(key)),
            "gpt3-5-turbo" => GAIEngines::Gpt3Dot5Turbo(ChatCompletionsClient::gpt3_5_turbo(key)),
            "gemini15flash" => {
                GAIEngines::Gemini15Flash(GeminiGenerateContent::gemini_15_flash(key))
            }
            "gemini2flashexp" => {
                GAIEngines::Gemini20FlashExp(GeminiGenerateContent::gemini_2_flash_exp(key))
            }
            "claude3-haiku" => GAIEngines::Claude3Haiku(ClaudeMessageClient::haiku_3(key)),
            "claude3-ops" => GAIEngines::Claude3Ops(ClaudeMessageClient::ops_3(key)),
            "claude35-sonnet" => GAIEngines::Claude35Sonnet(ClaudeMessageClient::sonnet_3_5(key)),
            "claude3-sonnet" => GAIEngines::Claude3Sonnet(ClaudeMessageClient::sonnet_3(key)),
            _ => GAIEngines::Gpt4oMini(ChatCompletionsClient::gpt4o_mini(key)),
        }
    }
}

pub fn engine_to_default_key_from_env(engine: &str) -> String {
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
    Gemini20FlashExp:GeminiGenerateContent,
    Claude3Haiku:ClaudeMessageClient,
    Claude3Ops:ClaudeMessageClient,
    Claude35Sonnet:ClaudeMessageClient,
    Claude3Sonnet:ClaudeMessageClient
);
