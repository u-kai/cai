use crate::{HandlerError, MutHandler};

pub struct Recorder {
    buf: String,
}

impl Recorder {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    pub fn message(&self) -> &str {
        self.buf.as_str()
    }
}

impl MutHandler for Recorder {
    async fn handle_mut(&mut self, resp: &str) -> Result<(), HandlerError> {
        self.buf.push_str(resp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[tokio::test]
    async fn recode_stream() {
        let mut sut = Recorder::new();

        let streams = vec!["1,", "2,", "3!"];

        for stream in streams {
            sut.handle_mut(stream).await.unwrap();
        }

        let message = sut.message();
        assert_eq!(message, "1,2,3!");
    }
}
