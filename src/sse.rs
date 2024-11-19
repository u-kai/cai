use anyhow::Context;
use std::future::Future;
use tokio_stream::StreamExt as _;

use crate::{impl_from_error, MutHandler};

pub struct SseClient {
    url: String,
    inner: reqwest::Client,
}

impl SseClient {
    pub fn new(url: &str) -> Self {
        let inner = reqwest::Client::new();
        SseClient {
            url: url.to_string(),
            inner,
        }
    }
    pub fn post(&self) -> RequestBuilder {
        self.inner.post(&self.url).into()
    }
}

pub struct RequestBuilder {
    builder: reqwest::RequestBuilder,
}
impl From<reqwest::RequestBuilder> for RequestBuilder {
    fn from(builder: reqwest::RequestBuilder) -> Self {
        RequestBuilder { builder }
    }
}
impl RequestBuilder {
    pub fn json<S: serde::Serialize>(mut self, data: S) -> Self {
        let body = serde_json::to_string(&data).unwrap();
        self.builder = self
            .builder
            .body(body)
            .header("Content-Type", "application/json");
        self
    }
    pub fn query(mut self, query: &[(&str, &str)]) -> Self {
        self.builder = self.builder.query(query);
        self
    }
    pub async fn request(self) -> Result<Response, reqwest::Error> {
        Ok(self.builder.send().await?.into())
    }
    pub fn bearer_auth(mut self, key: &str) -> Self {
        self.builder = self.builder.bearer_auth(key);
        self
    }
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.builder = self.builder.header(key, value);
        self
    }
}

pub struct Response {
    inner: reqwest::Response,
}

impl From<reqwest::Response> for Response {
    fn from(inner: reqwest::Response) -> Self {
        Response { inner }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SseHandleStreamError(anyhow::Error);
impl_from_error!(SseHandleStreamError, SseHandlerError);

impl Response {
    pub async fn handle_stream<H: SseHandler>(
        self,
        handler: &H,
    ) -> Result<(), SseHandleStreamError> {
        let mut stream = self.inner.bytes_stream();
        let mut reader = SseStreamReader::new();
        while let Some(bytes) = stream
            .next()
            .await
            .transpose()
            .context("Failed to read stream")?
        {
            let s = std::str::from_utf8(&bytes);
            match s {
                Ok(s) => {
                    tracing::info!("sse stream: {:?}", s);

                    let responses = reader.maybe_parse(s);
                    let Some(responses) = responses else {
                        continue;
                    };
                    for s in responses {
                        handler.handle(s).await.context("Failed to handle stream")?
                    }
                }
                Err(error) => {
                    println!("error: {:?}", error);
                }
            }
        }
        Ok(())
    }
    // TODO: remove and fix this func.
    pub async fn handle_mut_stream_use_convert<F, H>(
        self,
        f: F,
        handler: &mut H,
    ) -> Result<(), SseHandleStreamError>
    where
        F: Fn(SseResponse) -> Result<String, SseHandleStreamError>,
        H: MutHandler,
    {
        let mut stream = self.inner.bytes_stream();
        let mut reader = SseStreamReader::new();

        while let Some(bytes) = stream.next().await.transpose().unwrap() {
            let s = std::str::from_utf8(&bytes);
            match s {
                Ok(s) => {
                    tracing::info!("sse stream: {:?}", s);

                    let responses = reader.maybe_parse(s);
                    let Some(responses) = responses else {
                        continue;
                    };

                    for s in responses {
                        let s = f(s)?;
                        handler
                            .handle_mut(s.as_str())
                            .await
                            .context("Failed to handle stream")?
                    }
                }
                Err(error) => {
                    println!("error: {:?}", error);
                }
            }
        }
        Ok(())
    }
    pub async fn handle_mut_stream<H: SseMutHandler>(
        self,
        handler: &mut H,
    ) -> Result<(), SseHandleStreamError> {
        let mut stream = self.inner.bytes_stream();
        let mut reader = SseStreamReader::new();

        while let Some(bytes) = stream.next().await.transpose().unwrap() {
            let s = std::str::from_utf8(&bytes);
            match s {
                Ok(s) => {
                    tracing::info!("sse stream: {:?}", s);

                    let responses = reader.maybe_parse(s);
                    let Some(responses) = responses else {
                        continue;
                    };

                    for s in responses {
                        handler.handle(s).await.context("Failed to handle stream")?
                    }
                }
                Err(error) => {
                    println!("error: {:?}", error);
                }
            }
        }
        Ok(())
    }
}

pub trait SseHandler {
    #[allow(async_fn_in_trait)]
    async fn handle(&self, stream: SseResponse) -> Result<(), SseHandlerError>;
}
pub trait SseMutHandler {
    #[allow(async_fn_in_trait)]
    async fn handle(&mut self, stream: SseResponse) -> Result<(), SseHandlerError>;
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SseHandlerError(anyhow::Error);

impl<F, AsyncOutput> SseHandler for F
where
    AsyncOutput: Future<Output = Result<(), SseHandlerError>>,
    F: Fn(SseResponse) -> AsyncOutput,
{
    async fn handle(&self, stream: SseResponse) -> Result<(), SseHandlerError> {
        self(stream).await
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum SseResponse {
    Event(String),
    Data(String),
    Id(String),
    Retry(u32),
}

impl SseResponse {
    pub fn from_chunk(chunk: &str) -> Result<Vec<Self>, SseResponseParseError> {
        let mut result = vec![];
        let mut for_interrupted_data = vec![];

        //TODO: If newline (\n\n) is included in the normal data and not as a delimiter, it won't work properly.
        let delimiter = if chunk.contains("\r\n\r\n") {
            "\r\n\r\n"
        } else {
            "\n\n"
        };

        let lines = chunk.split(delimiter);

        for line in lines {
            for_interrupted_data.push(line);

            // If the line is empty, it is a delimiter.
            if line.is_empty() {
                continue;
            }

            if let Some(data) = Self::extract_str(line) {
                if let Self::Event(event) = data {
                    let mut event_maybe_data = event.split("\ndata: ");
                    match (event_maybe_data.next(), event_maybe_data.next()) {
                        (Some(event), Some(data)) => {
                            result.push(Self::Event(event.to_string()));
                            result.push(Self::Data(data.to_string()))
                        }
                        _ => result.push(Self::Event(event)),
                    }
                } else {
                    result.push(data);
                }
            } else {
                let result = for_interrupted_data.join(delimiter);
                return Err(SseResponseParseError::InterruptedData(result));
            }
        }
        // If the last character is a delimiter, it is good to divide.
        // In that case, the line becomes an empty string.
        // If the last line is not an empty string, perform a judgment because the delimiters are inappropriate.
        if let Some(last_line) = for_interrupted_data.last() {
            if !last_line.is_empty() {
                return Err(SseResponseParseError::InterruptedData(chunk.to_string()));
            }
        }
        Ok(result)
    }
    fn extract_str(line: &str) -> Option<Self> {
        if line.starts_with("data:") {
            return Some(Self::Data(Self::trim(line, "data:")));
        }
        if line.starts_with("event:") {
            return Some(Self::Event(Self::trim(line, "event:")));
        }
        if line.starts_with("id:") {
            return Some(Self::Id(Self::trim(line, "id:")));
        }
        if line.starts_with("retry:") {
            let retry = Self::trim(line, "retry:").parse();
            if let Ok(retry) = retry {
                return Some(Self::Retry(retry));
            }
        }
        None
    }
    pub fn data(&self) -> Option<&str> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }
    pub fn event(&self) -> Option<&str> {
        match self {
            Self::Event(event) => Some(event),
            _ => None,
        }
    }
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Id(id) => Some(id),
            _ => None,
        }
    }
    pub fn retry(&self) -> Option<u32> {
        match self {
            Self::Retry(retry) => Some(*retry),
            _ => None,
        }
    }
    fn trim(line: &str, res_type: &str) -> String {
        line.trim_start_matches(res_type).trim().to_string()
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SseResponseParseError {
    #[error("SseResponse InvalidFormat: {0}")]
    InvalidFormat(String),
    #[error("SseResponse InvalidRetry: {0}")]
    InvalidRetry(String),
    #[error("SseResponse InterruptedData: {0}")]
    InterruptedData(String),
}

struct SseStreamReader {
    interrupted_data: Option<String>,
    // TODO: Remove this field
    // This is a temporary field to prevent infinite loop
    call_time: usize,
}
impl SseStreamReader {
    fn new() -> Self {
        SseStreamReader {
            interrupted_data: None,
            call_time: 0,
        }
    }
    fn maybe_parse(&mut self, chunk: &str) -> Option<Vec<SseResponse>> {
        if let Some(interrupted_data) = self.interrupted_data.take() {
            self.call_time += 1;
            assert!(self.call_time < 10);

            let chunk = format!("{}{}", interrupted_data, chunk);
            return self.maybe_parse(chunk.as_str());
        }
        match SseResponse::from_chunk(chunk) {
            Ok(res) => Some(res),
            Err(SseResponseParseError::InterruptedData(data)) => {
                self.interrupted_data = Some(data);
                None
            }
            Err(e) => {
                println!("error: {:?}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    fn message(mes: &str) -> ChatRequest {
        ChatRequest {
            model: OpenAIModel::Gpt4o,
            messages: vec![Message {
                role: Role::User,
                content: mes.to_string(),
            }],
            stream: true,
        }
    }

    use super::*;

    pub struct GptHandler {
        has_received: bool,
    }
    impl GptHandler {
        pub fn new() -> Self {
            GptHandler {
                has_received: false,
            }
        }
    }
    impl SseHandler for GptHandler {
        async fn handle(&self, stream: SseResponse) -> Result<(), SseHandlerError> {
            assert!(stream.data().unwrap().len() > 0);
            Ok(())
        }
    }
    impl SseMutHandler for GptHandler {
        async fn handle(&mut self, stream: SseResponse) -> Result<(), SseHandlerError> {
            SseHandler::handle(self, stream).await.unwrap();
            self.has_received = true;
            Ok(())
        }
    }
    #[test]
    fn parse_sse_streams() {
        let mut sut = SseStreamReader::new();
        let stream = "event: content_block_delta\ndata: dddd\n\n";

        let chunk = sut.maybe_parse(stream);

        assert_eq!(
            chunk.unwrap(),
            vec![
                SseResponse::Event("content_block_delta".to_string()),
                SseResponse::Data("dddd".to_string())
            ]
        );

        let mut sut = SseStreamReader::new();
        let stream = "data: {\"id\":1}\r\n\r\nevent: message\r\n\r\ndata: {\"id\":2}\r\n\r\n";

        let chunk = sut.maybe_parse(stream);

        assert_eq!(
            chunk.unwrap(),
            vec![
                SseResponse::Data("{\"id\":1}".to_string()),
                SseResponse::Event("message".to_string()),
                SseResponse::Data("{\"id\":2}".to_string())
            ]
        );

        let mut sut = SseStreamReader::new();
        let stream = "data: {\"id\":1}\n\nevent: message\n\ndata: {\"id\":2}\n\n";
        let chunk = sut.maybe_parse(stream);
        assert_eq!(
            chunk.unwrap(),
            vec![
                SseResponse::Data("{\"id\":1}".to_string()),
                SseResponse::Event("message".to_string()),
                SseResponse::Data("{\"id\":2}".to_string())
            ]
        );
        let interrupted_stream = "data: {\"id\":1}\n\ndata:";
        let none = sut.maybe_parse(interrupted_stream);
        assert_eq!(none, None);

        let continuation_stream = " {\"id\":2}\n\n";
        let chunk = sut.maybe_parse(continuation_stream);
        assert_eq!(
            chunk.unwrap(),
            vec![
                SseResponse::Data("{\"id\":1}".to_string()),
                SseResponse::Data("{\"id\":2}".to_string())
            ]
        );
    }

    #[test]
    fn parse_sse_response() {
        let data = "data:{\"id\":1}\n\ndata:{\"id\":2}\n\n";
        let res = SseResponse::from_chunk(data).unwrap();
        assert_eq!(
            res,
            vec![
                SseResponse::Data("{\"id\":1}".to_string()),
                SseResponse::Data("{\"id\":2}".to_string())
            ]
        );
    }
    #[test]
    #[ignore]
    fn cases_where_sse_response_is_interrupted() {
        let data = "data:{\"id\":1}\n\nd";
        let err = SseResponse::from_chunk(data);
        assert_eq!(
            err,
            Err(SseResponseParseError::InterruptedData(
                "data:{\"id\":1}\n\nd".to_string()
            ))
        );

        let interrupted_stream = "data: {\"id\":1}\n\ndata:";
        let err = SseResponse::from_chunk(interrupted_stream);

        assert_eq!(
            err,
            Err(SseResponseParseError::InterruptedData(
                "data: {\"id\":1}\n\ndata:".to_string()
            ))
        );
    }
    #[tokio::test]
    #[ignore]
    async fn request_to_chatgpt() {
        let mut gpt_handler = GptHandler::new();
        let sut = SseClient::new("https://api.openai.com/v1/chat/completions");
        sut.post()
            .json(message("Hello"))
            .bearer_auth(&chatgpt_key())
            .request()
            .await
            .unwrap()
            .handle_mut_stream(&mut gpt_handler)
            .await
            .unwrap();
    }
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ChatRequest {
        model: OpenAIModel,
        messages: Vec<Message>,
        stream: bool,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Message {
        role: Role,
        content: String,
    }
    #[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
    pub enum Role {
        User,
        Assistant,
    }
    impl Role {
        fn into_str(&self) -> &'static str {
            match self {
                Self::User => "user",
                Self::Assistant => "assistant",
            }
        }
    }
    impl serde::Serialize for Role {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let role: &str = self.into_str();
            serializer.serialize_str(role)
        }
    }
    #[derive(Debug, Clone, serde::Deserialize)]
    pub enum OpenAIModel {
        Gpt3Dot5Turbo,
        Gpt4o,
    }
    impl serde::Serialize for OpenAIModel {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
        {
            serializer.serialize_str(self.into_str())
        }
    }

    impl OpenAIModel {
        pub fn into_str(&self) -> &'static str {
            match self {
                Self::Gpt3Dot5Turbo => "gpt-3.5-turbo",
                Self::Gpt4o => "gpt-4o",
            }
        }
    }
    impl From<OpenAIModel> for &'static str {
        fn from(model: OpenAIModel) -> Self {
            model.into_str()
        }
    }
    pub fn chatgpt_key() -> String {
        std::env::var("OPENAI_API_KEY").unwrap()
    }
}
