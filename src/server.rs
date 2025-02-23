use actix_web::{HttpResponse, HttpServer, Responder, web::Json};

use crate::{
    GenerativeAIInterface, Handler, HandlerError, MutHandler, Prompt,
    clients::{
        gai::{GAIEngines, engine_to_default_key_from_env},
        gemini::GeminiAPIClient,
        openai::GPTCompletionsClient,
    },
    container_handler,
    handlers::{printer::Printer, recorder::Recorder},
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
    let client = GeminiAPIClient::new(
        engine_to_default_key_from_env("gemini2flashexp"),
        crate::clients::gemini::GeminiModel::Gemini2FlashExp,
    );
    let prompt = Prompt::ask(body.prompt.as_str());
    let resp = client.request(prompt).await.unwrap();
    let resp = Response {
        result: resp.into(),
    };
    HttpResponse::Ok().body(serde_json::to_string(&resp).unwrap())
}
#[actix_web::post("/gemini2flashexp")]
async fn request_to_gemini2(body: Json<PromptRequest>) -> impl Responder {
    let client = GeminiAPIClient::new(
        engine_to_default_key_from_env("gemini2flashexp"),
        crate::clients::gemini::GeminiModel::Gemini2FlashExp,
    );
    let prompt = Prompt::ask(body.prompt.as_str());
    let resp = client.request(prompt).await.unwrap();
    let resp = Response {
        result: resp.into(),
    };
    HttpResponse::Ok().body(serde_json::to_string(&resp).unwrap())
}
#[actix_web::post("/gpt4o-mini")]
async fn request_to_gpt4omini(body: Json<PromptRequest>) -> impl Responder {
    let client = GPTCompletionsClient::new(
        engine_to_default_key_from_env("gpt4o-mini"),
        crate::clients::openai::ChatCompletionsModel::Gpt4oMini,
    );
    let prompt = Prompt::ask(body.prompt.as_str());
    let resp = client.request(prompt).await.unwrap();
    let resp = Response {
        result: resp.content(),
    };
    HttpResponse::Ok().body(serde_json::to_string(&resp).unwrap())
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
