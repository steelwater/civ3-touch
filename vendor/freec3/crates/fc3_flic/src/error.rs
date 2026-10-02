#[derive(Debug)]
pub enum FlicError {
    InvalidHeader,
    InvalidDimensions,
    UnsupportedType,
    UnsupportedFormat,
    CorruptedChunk,
    UnexpectedEndOfFile,
}

impl core::fmt::Display for FlicError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FlicError::InvalidHeader => write!(f, "Invalid FLIC header"),
            FlicError::InvalidDimensions => write!(f, "Invalid FLIC dimensions"),
            FlicError::UnsupportedType => write!(f, "Unsupported FLIC type"),
            FlicError::UnsupportedFormat => write!(f, "Unsupported FLIC format"),
            FlicError::CorruptedChunk => write!(f, "Corrupted FLIC chunk"),
            FlicError::UnexpectedEndOfFile => write!(f, "Unexpected end of file"),
        }
    }
}

impl std::error::Error for FlicError {}
