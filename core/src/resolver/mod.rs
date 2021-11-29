#[derive(Debug, strum::Display)]
pub enum ResolverError {
    NoEntry,
}

impl std::error::Error for ResolverError {}
