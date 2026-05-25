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
