use actix_web::{web::Json, HttpResponse, HttpServer, Responder};

use crate::{
    clients::gai::{engine_to_default_key_from_env, GAIEngines},
    handlers::recorder::Recorder,
    GenerativeAIInterface, Prompt,
};

pub struct AIServer {
    port: u16,
}

impl AIServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(self) {
        HttpServer::new(|| actix_web::App::new().service(request_to))
            .bind(("localhost", self.port))
            .unwrap()
            .run()
            .await
            .unwrap();
    }
}

#[actix_web::post("/")]
async fn request_to(body: Json<PromptRequest>) -> impl Responder {
    let mut handler = Recorder::new();
    let ai = GAIEngines::from_str("gpt4-o", engine_to_default_key_from_env("gpt4-o"));
    let prompt = Prompt::ask(&body.prompt);
    ai.request_mut(prompt, &mut handler).await.unwrap();
    let response = handler.take();
    HttpResponse::Ok().body(serde_json::to_string(&Response { result: response }).unwrap())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Response {
    result: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PromptRequest {
    prompt: String,
}
