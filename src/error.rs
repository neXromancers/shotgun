
#[derive(Debug)]
pub enum CaptureError {
    DisplayOpen,
    InvalidWindowID(String),
    InvalidGeometry,
    FailedToCaptureFromX11,
    UnableToConvertFramebuffer,
    FailedToEnumerateScreens,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CaptureError::*;
        match self {
            DisplayOpen => f.write_str( "Failed to open display"),
            InvalidWindowID(id) => write!(f, "Window ID {:?} is not a valid integer\nAccepted values are decimal, hex (0x*), octal (0o*) and binary (0b*)", id),
            InvalidGeometry => f.write_str("Invalid geometry"),
            FailedToCaptureFromX11 => f.write_str("Failed to get image from X"),
            UnableToConvertFramebuffer => f.write_str("Failed to convert captured framebuffer, only 24/32 bit (A)RGB8 is supported"),
            FailedToEnumerateScreens => f.write_str("Failed to enumerate screens, not masking"),
        }
    }
}