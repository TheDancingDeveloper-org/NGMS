use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::error::{StreamError, StreamResult};

/// Parsed HTTP Range header for a single byte range.
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64, // inclusive
}

/// Response data for direct file serving.
pub struct DirectPlayResponse {
    /// HTTP status code (200 or 206).
    pub status: u16,
    /// Content-Type header value.
    pub content_type: String,
    /// Content-Length of the response body.
    pub content_length: u64,
    /// Content-Range header (only for 206 responses).
    pub content_range: Option<String>,
    /// Total file size.
    pub file_size: u64,
    /// Streaming body.
    pub body: ReaderStream<tokio::io::Take<tokio::fs::File>>,
}

/// Serve a media file with HTTP range request support.
///
/// `range_header` should be the raw value of the `Range` HTTP header, if present.
pub async fn serve_file(
    file_path: &Path,
    range_header: Option<&str>,
) -> StreamResult<DirectPlayResponse> {
    let mut file = tokio::fs::File::open(file_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            StreamError::NotFound(file_path.display().to_string())
        } else {
            StreamError::Io(e)
        }
    })?;

    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    let content_type = mime_guess::from_path(file_path)
        .first_or_octet_stream()
        .to_string();

    match range_header {
        Some(range_str) => {
            let range = parse_range(range_str, file_size)?;

            file.seek(std::io::SeekFrom::Start(range.start)).await?;
            let length = range.end - range.start + 1;
            let take = file.take(length);

            Ok(DirectPlayResponse {
                status: 206,
                content_type,
                content_length: length,
                content_range: Some(format!(
                    "bytes {}-{}/{}",
                    range.start, range.end, file_size
                )),
                file_size,
                body: ReaderStream::new(take),
            })
        }
        None => {
            let take = file.take(file_size);
            Ok(DirectPlayResponse {
                status: 200,
                content_type,
                content_length: file_size,
                content_range: None,
                file_size,
                body: ReaderStream::new(take),
            })
        }
    }
}

/// Parse a single-range `Range: bytes=start-end` header.
fn parse_range(header: &str, file_size: u64) -> StreamResult<ByteRange> {
    let s = header
        .strip_prefix("bytes=")
        .ok_or_else(|| StreamError::InvalidRange("missing bytes= prefix".into()))?;

    // Only handle a single range (no multi-range)
    let range_spec = s
        .split(',')
        .next()
        .ok_or_else(|| StreamError::InvalidRange("empty range".into()))?
        .trim();

    let parts: Vec<&str> = range_spec.splitn(2, '-').collect();
    if parts.len() != 2 {
        return Err(StreamError::InvalidRange(format!(
            "invalid range format: {range_spec}"
        )));
    }

    let (start_str, end_str) = (parts[0].trim(), parts[1].trim());

    if start_str.is_empty() {
        // Suffix range: bytes=-500 means last 500 bytes
        let suffix_len: u64 = end_str
            .parse()
            .map_err(|_| StreamError::InvalidRange(format!("invalid suffix: {end_str}")))?;
        if suffix_len == 0 || suffix_len > file_size {
            return Err(StreamError::InvalidRange(
                "suffix length out of range".into(),
            ));
        }
        Ok(ByteRange {
            start: file_size - suffix_len,
            end: file_size - 1,
        })
    } else {
        let start: u64 = start_str
            .parse()
            .map_err(|_| StreamError::InvalidRange(format!("invalid start: {start_str}")))?;

        let end = if end_str.is_empty() {
            // Open-ended range: bytes=500-
            file_size - 1
        } else {
            let e: u64 = end_str
                .parse()
                .map_err(|_| StreamError::InvalidRange(format!("invalid end: {end_str}")))?;
            e.min(file_size - 1)
        };

        if start > end || start >= file_size {
            return Err(StreamError::InvalidRange(format!(
                "range {start}-{end} not satisfiable for file size {file_size}"
            )));
        }

        Ok(ByteRange { start, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_full() {
        let r = parse_range("bytes=0-999", 10000).unwrap();
        assert_eq!(r.start, 0);
        assert_eq!(r.end, 999);
    }

    #[test]
    fn test_parse_range_open_ended() {
        let r = parse_range("bytes=500-", 10000).unwrap();
        assert_eq!(r.start, 500);
        assert_eq!(r.end, 9999);
    }

    #[test]
    fn test_parse_range_suffix() {
        let r = parse_range("bytes=-500", 10000).unwrap();
        assert_eq!(r.start, 9500);
        assert_eq!(r.end, 9999);
    }

    #[test]
    fn test_parse_range_clamped() {
        let r = parse_range("bytes=0-99999", 10000).unwrap();
        assert_eq!(r.start, 0);
        assert_eq!(r.end, 9999);
    }

    #[test]
    fn test_parse_range_invalid() {
        assert!(parse_range("bytes=9999-0", 10000).is_err());
        assert!(parse_range("bytes=20000-", 10000).is_err());
        assert!(parse_range("invalid", 10000).is_err());
    }
}
