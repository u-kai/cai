use actix_web::{web::Json, FromRequest, HttpResponse, HttpServer, Responder};

use crate::{
    clients::gai::{engine_to_default_key_from_env, GAIEngines},
    container_handler,
    handlers::printer::Printer,
    handlers::recorder::Recorder,
    GenerativeAIInterface, Handler, HandlerError, MutHandler, Prompt,
};

pub struct AIServer {
    port: u16,
}

impl AIServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(self) {
        HttpServer::new(|| {
            let cors = actix_cors::Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);
            actix_web::App::new()
                .service(request_to)
                .service(request_to_gemini2)
                .service(request_to_gemini15)
                .service(request_to_gpt4omini)
                .wrap(cors)
        })
        .bind(("localhost", self.port))
        .unwrap()
        .run()
        .await
        .unwrap();
    }
}

#[actix_web::post("/")]
async fn request_to(body: Json<PromptRequest>) -> impl Responder {
    let res = handle_prompt::<Response>("gemini2flashexp", &body.prompt).await;
    HttpResponse::Ok().body(serde_json::to_string(&res).unwrap())
}
#[actix_web::post("/gemini2flashexp")]
async fn request_to_gemini2(body: Json<PromptRequest>) -> impl Responder {
    let res = handle_prompt::<Response>("gemini2flashexp", &body.prompt).await;
    HttpResponse::Ok().body(serde_json::to_string(&res).unwrap())
}
#[actix_web::post("/gpt4o-mini")]
async fn request_to_gpt4omini(body: Json<PromptRequest>) -> impl Responder {
    let res = handle_prompt::<Response>("gpt4-o-mini", &body.prompt).await;
    HttpResponse::Ok().body(serde_json::to_string(&res).unwrap())
}

#[actix_web::post("/gemini15flash")]
async fn request_to_gemini15(body: Json<PromptRequest>) -> impl Responder {
    let res = handle_prompt::<Response>("gemini15flash", &body.prompt).await;
    HttpResponse::Ok().body(serde_json::to_string(&res).unwrap())
}

async fn handle_prompt<T: From<String>>(name: &str, prompt: &str) -> T {
    container_handler!(recorder:Recorder,printer:Printer);
    let mut handler = Container {
        recorder: Recorder::new(),
        printer: Printer::new(),
    };
    let ai = GAIEngines::from_str(name, engine_to_default_key_from_env(name));
    let prompt = Prompt::ask(prompt);
    ai.request_mut(prompt, &mut handler).await.unwrap();
    let response = handler.recorder.take();
    T::from(response)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Response {
    result: String,
}
impl From<String> for Response {
    fn from(s: String) -> Self {
        Self { result: s }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PromptRequest {
    prompt: String,
}
