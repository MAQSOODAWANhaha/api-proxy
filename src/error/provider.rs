use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Placeholder provider error")]
    Placeholder,
}
