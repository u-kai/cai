pub mod claude;
pub mod genemi;
pub mod openai;

#[cfg(test)]
pub mod mocks {
    use crate::{Handler, HandlerError, MutHandler};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MockHandler {
        pub has_received: bool,
        pub received: String,
    }
    impl MockHandler {
        pub fn new() -> Self {
            MockHandler {
                has_received: false,
                received: "".to_string(),
            }
        }
    }
    impl Handler for MockHandler {
        async fn handle(&self, stream: &str) -> Result<(), HandlerError> {
            println!("Received: {}", stream);
            Ok(())
        }
    }
    impl MutHandler for MockHandler {
        async fn handle_mut(&mut self, stream: &str) -> Result<(), HandlerError> {
            self.received += stream;
            self.has_received = true;
            Ok(())
        }
    }
}