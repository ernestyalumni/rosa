//! Minimal SSE (Server-Sent Events) parser for streaming LLM responses.
//!
//! Follows RFC 8898 §9: accumulate lines, dispatch on blank-line event boundaries.
//! Silently skips `id:`, `retry:`, and comment (`:`) lines.
//!
//! Usage:
//! ```ignore
//! let events = sse_events(response.bytes_stream());
//! while let Some((event_type, data)) = events.next().await { ... }
//! ```

use async_stream::stream;
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};

/// Transform a raw reqwest byte stream into `(event_type, data)` tuples.
///
/// - `event_type` is `"message"` when no `event:` line precedes the data.
/// - Multiple `data:` lines are joined with `\n`.
/// - Errors in the underlying byte stream are logged and skipped.
pub fn sse_events(
    byte_stream: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
) -> impl Stream<Item = (String, String)> + Send + 'static {
    stream! {
        let mut buf = String::new();
        let mut event_type = String::from("message");
        let mut data_lines: Vec<String> = Vec::new();

        let mut byte_stream = Box::pin(byte_stream);

        while let Some(result) = byte_stream.next().await {
            let bytes = match result {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "SSE stream read error — skipping chunk");
                    continue;
                }
            };

            // SSE is always UTF-8; lossy conversion handles partial multi-byte seqs
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // Drain all complete lines from the buffer
            loop {
                match buf.find('\n') {
                    None => break,
                    Some(nl) => {
                        // Trim optional CR (Windows line endings)
                        let line = buf[..nl].trim_end_matches('\r').to_owned();
                        buf = buf[nl + 1..].to_owned();

                        if line.is_empty() {
                            // Blank line = event boundary
                            if !data_lines.is_empty() {
                                yield (event_type.clone(), data_lines.join("\n"));
                                data_lines.clear();
                                event_type = String::from("message");
                            }
                        } else if let Some(ev) = line.strip_prefix("event:") {
                            event_type = ev.trim_start().to_owned();
                        } else if let Some(d) = line.strip_prefix("data:") {
                            data_lines.push(d.trim_start().to_owned());
                        }
                        // Ignore: id:, retry:, comment lines (:...)
                    }
                }
            }
        }

        // Flush any event that ended without a trailing blank line
        if !data_lines.is_empty() {
            yield (event_type, data_lines.join("\n"));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::sse_events;
    use bytes::Bytes;
    use futures::StreamExt;

    /// Feed static byte slices into `sse_events` and collect all events.
    ///
    /// `b"..."` literals are `&'static [u8]`, so `Bytes::from_static` is
    /// zero-copy and the resulting stream satisfies the `'static` bound.
    async fn parse(chunks: &[&'static [u8]]) -> Vec<(String, String)> {
        let items: Vec<reqwest::Result<Bytes>> = chunks
            .iter()
            .map(|&b| Ok(Bytes::from_static(b)))
            .collect();
        sse_events(futures::stream::iter(items)).collect().await
    }

    // ── basic parsing ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_single_data_event() {
        let events = parse(&[b"data: hello\n\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "hello".to_owned())]);
    }

    #[tokio::test]
    async fn test_named_event_type() {
        // event: field sets the type for the next dispatch
        let events = parse(&[b"event: ping\ndata: {}\n\n"]).await;
        assert_eq!(events, vec![("ping".to_owned(), "{}".to_owned())]);
    }

    #[tokio::test]
    async fn test_multiple_data_lines_joined_with_newline() {
        // RFC 8898: multiple data: lines are concatenated with \n
        let events = parse(&[b"data: line1\ndata: line2\n\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "line1\nline2".to_owned())]);
    }

    // ── line-ending variants ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_crlf_line_endings_stripped() {
        // Windows-style \r\n is the same as \n (CR stripped)
        let events = parse(&[b"data: hello\r\n\r\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "hello".to_owned())]);
    }

    // ── ignored field types ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_id_and_retry_lines_ignored() {
        let events = parse(&[b"id: 123\nretry: 1000\ndata: payload\n\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "payload".to_owned())]);
    }

    #[tokio::test]
    async fn test_comment_lines_ignored() {
        // Lines starting with ':' are SSE comments
        let events = parse(&[b": this is a comment\ndata: real\n\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "real".to_owned())]);
    }

    // ── boundary conditions ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_blank_line_without_data_does_not_yield() {
        // Multiple blank lines before actual data → only one event at the end
        let events = parse(&[b"\n\n\ndata: actual\n\n"]).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "actual");
    }

    #[tokio::test]
    async fn test_no_trailing_blank_line_flushed_at_stream_end() {
        // Stream ends without the closing blank line — still yields
        let events = parse(&[b"data: eof\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "eof".to_owned())]);
    }

    // ── multi-event streams ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_multiple_events_in_sequence() {
        let events = parse(&[b"data: first\n\ndata: second\n\n"]).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].1, "first");
        assert_eq!(events[1].1, "second");
    }

    #[tokio::test]
    async fn test_event_type_resets_to_message_after_each_event() {
        // event: foo applies only to the NEXT event, then resets to "message"
        let events = parse(&[b"event: foo\ndata: 1\n\ndata: 2\n\n"]).await;
        assert_eq!(events[0], ("foo".to_owned(),     "1".to_owned()));
        assert_eq!(events[1], ("message".to_owned(), "2".to_owned()));
    }

    // ── chunked delivery ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_bytes_split_mid_line_buffered_correctly() {
        // Simulate TCP fragmentation: "data: hel" arrives first, then "lo\n\n"
        let events = parse(&[b"data: hel", b"lo\n\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "hello".to_owned())]);
    }

    #[tokio::test]
    async fn test_event_boundary_split_across_chunks() {
        // First chunk ends exactly on the first \n of the blank-line boundary
        let events = parse(&[b"data: x\n", b"\n"]).await;
        assert_eq!(events, vec![("message".to_owned(), "x".to_owned())]);
    }
}
