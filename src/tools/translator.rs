use std::fmt::Display;

use crate::{handlers::recorder::Recorder, AIError, GenerativeAIInterface, Prompt};

pub async fn translate<AI: GenerativeAIInterface>(
    ai: AI,
    request: TranslateRequests,
) -> Result<Vec<TranslateResult>, AIError> {
    let requests = request.to_requests();
    let tasks = requests.into_iter().map(|req| translate_task(&ai, req));
    Ok(futures::future::join_all(tasks)
        .await
        .into_iter()
        .filter_map(|res| res.ok())
        .collect())
}

async fn translate_task<AI: GenerativeAIInterface>(
    ai: &AI,
    request: TranslateRequest,
) -> Result<TranslateResult, AIError> {
    let mut recorder = Recorder::new();
    ai.request_mut(request.to_prompt(), &mut recorder).await?;
    Ok(TranslateResult {
        from: request,
        translated: recorder.take(),
    })
}

pub struct TranslateResult {
    from: TranslateRequest,
    translated: String,
}
impl Display for TranslateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}", self.from.source, self.translated)
    }
}
#[derive(Debug, PartialEq)]
struct TranslateRequest {
    source: String,
    target_lang: TargetLang,
}
impl TranslateRequest {
    fn to_prompt(&self) -> Prompt {
        Prompt::ask(&format!(
        "please translate '{}' to {}. you should answer only in the target language and result. If there is something like program code in the translation target, please ignore it and output it as is.",
        self.source,
        self.target_lang.to_str()
    ))
    }
}

pub struct TranslateRequests {
    source: String,
    // if you want to separate the source string by some characters, set them here.
    separators: Vec<char>,
    // if you set this value to 2, and source string is "hello, world! Are you okay?", and separators is [',','?','!']
    // then, the source string will be separated into ["hello, world!", "Are you okay?"]
    // first ',' is counted as 1, and second '!' is counted as 2 and separate_per_limit is 2, so the source string is separated.
    separate_per_limit: usize,
    target_lang: TargetLang,
}

impl TranslateRequests {
    pub fn new(source: String, target_lang: TargetLang) -> Self {
        Self {
            source,
            separate_per_limit: 1,
            separators: vec![],
            target_lang,
        }
    }
    pub fn separate_per_limit(mut self, limit: usize) -> Self {
        self.separate_per_limit = limit;
        self
    }
    pub fn separators(mut self, separators: Vec<char>) -> Self {
        self.separators = separators;
        self
    }

    fn to_requests(self) -> Vec<TranslateRequest> {
        if self.separators.is_empty() {
            return vec![TranslateRequest {
                source: self.source,
                target_lang: self.target_lang,
            }];
        }
        self.source
            .split_inclusive(|c| self.separators.contains(&c))
            .fold(vec![], |mut acc, sentence| {
                if sentence.is_empty() {
                    return acc;
                }
                if acc.is_empty() {
                    acc.push(sentence.to_string());
                    return acc;
                }
                let last = acc.pop().unwrap();
                let split_count = last.chars().filter(|c| self.separators.contains(c)).count();
                // if the sentence not starts with a space, it should be concatenated with the last sentence.
                // for example, "app.Backend" should be concatenated "app" and ".Backend"
                if !sentence.starts_with(' ') {
                    acc.push(format!("{}{}", last, sentence));
                    return acc;
                }
                if split_count < self.separate_per_limit {
                    acc.push(format!("{}{}", last, sentence));
                } else {
                    acc.push(last);
                    // sentence is started with a space, so it should be trimmed.
                    acc.push(sentence.trim().to_string());
                }
                acc
            })
            .into_iter()
            .map(|sentence| TranslateRequest {
                source: sentence,
                target_lang: self.target_lang,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetLang {
    English,
    Japanese,
}
impl TargetLang {
    pub fn to_str(&self) -> &str {
        match self {
            TargetLang::English => "en",
            TargetLang::Japanese => "ja",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translate_request_should_not_separate_separators_after_not_empty_char() {
        let request = TranslateRequests::new(
            "hello, world! Are you okay? app.NotSplit".to_string(),
            TargetLang::Japanese,
        )
        .separate_per_limit(1)
        .separators(vec![',', '?', '!', '.']);
        let requests = request.to_requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[0],
            TranslateRequest {
                source: "hello,".to_string(),
                target_lang: TargetLang::Japanese
            },
        );
        assert_eq!(
            requests[1],
            TranslateRequest {
                source: "world!".to_string(),
                target_lang: TargetLang::Japanese
            },
        );
        assert_eq!(
            requests[2],
            TranslateRequest {
                source: "Are you okay?".to_string(),
                target_lang: TargetLang::Japanese
            },
        );
        assert_eq!(
            requests[3],
            TranslateRequest {
                source: "app.NotSplit".to_string(),
                target_lang: TargetLang::Japanese
            },
        );
    }
    #[test]
    fn translate_request_should_separate_source_string() {
        let request = TranslateRequests::new(
            "hello, world! Are you okay?".to_string(),
            TargetLang::Japanese,
        )
        .separate_per_limit(2)
        .separators(vec![',', '?', '!']);
        let requests = request.to_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0],
            TranslateRequest {
                source: "hello, world!".to_string(),
                target_lang: TargetLang::Japanese
            }
        );
        assert_eq!(
            requests[1],
            TranslateRequest {
                source: "Are you okay?".to_string(),
                target_lang: TargetLang::Japanese
            }
        );
    }
}
