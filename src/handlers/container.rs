#[macro_export]
macro_rules! container_handler {
    ($($name:ident:$t:ty),*) => {

        /// A container for multiple handlers.
        /// This is useful when you want to use multiple handlers at the same time.
        struct Container {
            $($name: $t,)*
        }
        impl MutHandler for Container {
            async fn handle_mut(&mut self, resp: &str) -> Result<(), HandlerError> {
                $(
                    self.$name.handle_mut(resp).await?;
                )*
                Ok(())
            }
        }
        impl Handler for Container {
            async fn handle(&self, resp: &str) -> Result<(), HandlerError> {
                Ok(())
            }
        }
    };
}
