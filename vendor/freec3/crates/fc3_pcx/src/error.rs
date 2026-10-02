#[derive(Debug)]
pub enum PcxError {
    InvalidSignature,
    UnsupportedEncoding,
    InvalidHeader,
    InvalidDimensions,
    InvalidPalette,
    DecodingError,
}

impl core::fmt::Display for PcxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PcxError::InvalidSignature => write!(f, "Invalid PCX signature"),
            PcxError::UnsupportedEncoding => write!(f, "Unsupported encoding method"),
            PcxError::InvalidHeader => write!(f, "Invalid PCX header"),
            PcxError::InvalidDimensions => write!(f, "Invalid image dimensions"),
            PcxError::InvalidPalette => write!(f, "Invalid or missing palette"),
            PcxError::DecodingError => write!(f, "Error decoding PCX data"),
        }
    }
}

impl std::error::Error for PcxError {}
