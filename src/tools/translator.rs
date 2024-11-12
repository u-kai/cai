use crate::{handlers::recorder::Recorder, AIError, GenerativeAIInterface, Prompt};

pub async fn translate<AI: GenerativeAIInterface>(
    ai: AI,
    request: TranslateRequest,
) -> Result<TranslateResult, AIError> {
    let mut recorder = Recorder::new();
    let prompts = request.to_prompts();
    ai.request_mut(prompts[0].clone(), &mut recorder).await?;
    Ok(TranslateResult {
        from: request,
        translated: recorder.take(),
    })
}

pub struct TranslateResult {
    pub from: TranslateRequest,
    pub translated: String,
}

pub struct TranslateRequest {
    source: String,
    // if you want to separate the source string by some characters, set them here.
    separators: Vec<char>,
    // if you set this value to 2, and source string is "hello, world! Are you okay?", and separators is [',','?','!']
    // then, the source string will be separated into ["hello, world!", "Are you okay?"]
    // first ',' is counted as 1, and second '!' is counted as 2 and separate_per_limit is 2, so the source string is separated.
    separate_per_limit: usize,
    target_lang: TargetLang,
}

impl TranslateRequest {
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

    pub fn to_prompts(&self) -> Vec<Prompt> {
        if self.separators.is_empty() {
            return vec![translate_prompt(&self.source, self.target_lang)];
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
                if split_count < self.separate_per_limit {
                    acc.push(format!("{}{}", last, sentence));
                } else {
                    acc.push(last);
                    acc.push(sentence.trim().to_string());
                }
                acc
            })
            .into_iter()
            .map(|sentence| translate_prompt(sentence.as_str(), self.target_lang))
            .collect()
    }
}

fn translate_prompt(source: &str, target_lang: TargetLang) -> Prompt {
    Prompt::ask(&format!(
        "please translate '{}' to {}. you should answer only in the target language and result.",
        source,
        target_lang.to_str()
    ))
}
#[derive(Debug, Clone, Copy)]
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
    fn translate_request_should_separate_source_string() {
        let request = TranslateRequest::new(
            "hello, world! Are you okay?".to_string(),
            TargetLang::Japanese,
        )
        .separate_per_limit(2)
        .separators(vec![',', '?', '!']);
        let prompts = request.to_prompts();
        assert_eq!(prompts.len(), 2);
        assert_eq!(
            prompts[0],
            translate_prompt("hello, world!", TargetLang::Japanese)
        );
        assert_eq!(
            prompts[1],
            translate_prompt("Are you okay?", TargetLang::Japanese)
        );
    }
}
