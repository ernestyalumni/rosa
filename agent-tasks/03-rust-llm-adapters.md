# Task 03 — LLM Provider Adapters

**Owner role:** Rust agent
**Blocked by:** 02 (needs `rosa-core` traits + `Message` + `AgentEvent`)
**Blocks:** 06
**Parallel with:** 04, 05
**Estimate:** 3–5 hr per adapter (OpenAI first, then Anthropic, then Ollama)

## Goal

Implement `crates/rosa-llm/` exposing the `LlmProvider` trait from `rosa-core` for: (a) OpenAI / OpenAI-compatible servers, (b) Anthropic Messages, (c) Ollama. Streaming tool calls supported. No `async-openai` / `anthropic-sdk` deps — hand-roll on `reqwest` + `eventsource-stream`.

## Context — why hand-rolled

The off-the-shelf Rust SDKs ship their own retry, error, and event types that conflict with `rosa-core`'s `AgentEvent`. Each adapter is ~400 lines including tests. The benefit of owning the wire layer: prompt-cache headers (Anthropic), reasoning effort knobs (OpenAI), and Ollama's non-standard `/api/chat` deltas all just work without fighting an SDK abstraction.

## Files to create

```
crates/rosa-llm/
├── Cargo.toml
└── src/
    ├── lib.rs           # re-exports the three adapters + a `make_provider(url, key, model)` factory
    ├── common/
    │   ├── mod.rs
    │   ├── sse.rs       # SSE chunk parser used by OpenAI + Anthropic
    │   └── retry.rs     # exponential backoff w/ jitter; 429/5xx only
    ├── openai.rs        # OpenAI + Azure + any OpenAI-compatible base URL
    ├── anthropic.rs     # Anthropic Messages API (incl. prompt caching)
    └── ollama.rs        # local Ollama
```

## Deps

```toml
rosa-core = { path = "../rosa-core" }
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }
eventsource-stream = "0.2"
async-trait = "0.1"
futures = "0.3"
tokio = { version = "1", features = ["sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
url = "2"

[dev-dependencies]
wiremock = "0.6"
tokio = { version = "1", features = ["macros"] }
```

## Wire-format references

- OpenAI: <https://platform.openai.com/docs/api-reference/chat/streaming> — pay attention to `tool_calls` arriving as deltas indexed by position; you must accumulate them across chunks.
- Anthropic: <https://docs.anthropic.com/en/api/messages-streaming> — `content_block_start` / `content_block_delta` for `tool_use` blocks; `cache_control` header for prompt caching.
- Ollama: <https://github.com/ollama/ollama/blob/main/docs/api.md#generate-a-chat-completion> — `/api/chat` returns one JSON object per line (NDJSON), not SSE.

## Tool-call accumulation gotcha

OpenAI streams tool calls like this (simplified):

```
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","function":{"name":"get_topics","arguments":""}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pat"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"tern\":\".*\"}"}}]}}]}
data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}
```

You accumulate by `(index, field)`; emit `ChatChunk::ToolCall { args_delta }` per delta; emit `Finish` only on `finish_reason`. Test this explicitly with a `wiremock` fixture.

## Anthropic prompt caching

For long system prompts (which `rosa-core` will have once tools are registered), add `cache_control: { type: "ephemeral" }` to the system message. Inspired-by reference: `/home/propdev/.openclaw/workspace/workspace2/repos/hermes-agent/agent/prompt_caching.py`. **Read it**, don't copy — the Python's logic is non-trivial.

## Acceptance criteria

- [ ] `cargo test -p rosa-llm` passes with wiremock-backed tests for each adapter covering: (i) streaming text, (ii) streaming a single tool call, (iii) streaming two parallel tool calls, (iv) 429 retry, (v) malformed SSE → `RosaError::Provider`.
- [ ] No panics on partial SSE buffers split mid-line.
- [ ] Provider keys read from `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` env (no config file path).
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] Manual smoke test documented in PR: hit a real OpenAI/Anthropic endpoint with `cargo run --example smoke -- --provider openai --model gpt-4o-mini` and observe tokens stream.

## Out of scope

- Bedrock, Gemini, Cohere — they're easy follow-ups but not needed for turtle/Starship demos.
- Tool definition / dispatch — that's task 04. This crate only translates `rosa-core::Message` ↔ provider wire format and exposes raw `ChatChunk`s.
