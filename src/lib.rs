use std::io;
use std::io::Read;

use encoding_rs_io::DecodeReaderBytesBuilder;
use memchr::memchr;

/// Result of classifying a byte stream as probably text or probably binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Probably text.
    Text,
    /// Probably binary.
    Binary,
}

impl Kind {
    #[inline]
    pub fn is_binary(self) -> bool {
        matches!(self, Kind::Binary)
    }

    #[inline]
    pub fn code_byte(self) -> u8 {
        match self {
            Kind::Text => b'0',
            Kind::Binary => b'1',
        }
    }
}

/// Classifies files/streams using a ripgrep-like heuristic: if any NUL byte is
/// observed while reading, treat the input as binary.
///
/// This type is reusable to avoid reallocating the read buffer when classifying
/// multiple files.
#[derive(Debug)]
pub struct Classifier {
    buf: Vec<u8>,
    decode_builder: DecodeReaderBytesBuilder,
    decode_buffer: Vec<u8>,
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier {
    /// The chunk size used by the classifier. This matches ripgrep's default
    /// line buffer capacity.
    pub const DEFAULT_CHUNK_SIZE: usize = 64 * (1 << 10);

    pub fn new() -> Self {
        let mut decode_builder = DecodeReaderBytesBuilder::new();
        decode_builder
            .utf8_passthru(true)
            .strip_bom(true)
            .bom_override(true)
            .bom_sniffing(true);
        Self {
            buf: vec![0; Self::DEFAULT_CHUNK_SIZE],
            decode_builder,
            decode_buffer: vec![0; 8 * (1 << 10)],
        }
    }

    /// Classify a reader by streaming through it until a NUL byte is found or
    /// EOF is reached.
    pub fn classify_reader<R: io::Read>(
        &mut self,
        reader: R,
    ) -> io::Result<Kind> {
        // Mirror ripgrep's approach: perform BOM sniffing/transcoding before
        // binary detection so UTF-16 (with BOM) is treated as text.
        let (buf, decode_buffer) = (&mut self.buf, &mut self.decode_buffer);
        let mut decoder = self
            .decode_builder
            .build_with_buffer(reader, decode_buffer)?;

        loop {
            let n = decoder.read(buf)?;
            if n == 0 {
                return Ok(Kind::Text);
            }
            if memchr(0, &buf[..n]).is_some() {
                return Ok(Kind::Binary);
            }
        }
    }

    /// Classify a file by path.
    pub fn classify_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> io::Result<Kind> {
        let file = std::fs::File::open(path)?;
        self.classify_reader(file)
    }
}

/// Classify the given bytes without any I/O.
#[inline]
pub fn classify_bytes(bytes: &[u8]) -> Kind {
    if memchr(0, bytes).is_some() {
        Kind::Binary
    } else {
        Kind::Text
    }
}

/// Convenience: returns `Ok(true)` if the path is probably binary.
pub fn is_binary_path<P: AsRef<std::path::Path>>(path: P) -> io::Result<bool> {
    let mut classifier = Classifier::new();
    Ok(classifier.classify_path(path)?.is_binary())
}
