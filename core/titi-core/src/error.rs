#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("frame too short: need {need}, got {got}")]
    Truncated { need: usize, got: usize },
    #[error("unsupported protocol version {0}")]
    Version(u8),
    #[error("unknown frame type {0:#04x}")]
    FrameType(u8),
    #[error("invalid field: {0}")]
    Invalid(&'static str),
    #[error("crypto failure: {0}")]
    Crypto(&'static str),
    #[error("noise: {0}")]
    Noise(String),
    #[error("replayed or out-of-window sequence")]
    Replay,
    #[error("unknown peer")]
    UnknownPeer,
    #[error("unknown group")]
    UnknownGroup,
    #[error("invite code invalid or expired")]
    InviteCode,
    #[error("protobuf decode: {0}")]
    Proto(String),
    #[error("codec: {0}")]
    Codec(String),
    #[error("floor busy")]
    FloorBusy,
}

impl From<prost::DecodeError> for Error {
    fn from(e: prost::DecodeError) -> Self {
        Error::Proto(e.to_string())
    }
}

impl From<snow::Error> for Error {
    fn from(e: snow::Error) -> Self {
        Error::Noise(format!("{e:?}"))
    }
}
