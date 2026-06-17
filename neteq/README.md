# NetEq

This is a local, trimmed copy of the `videocall-rs` NetEQ-inspired adaptive jitter buffer.

The crate is intentionally focused on pure Rust jitter-buffer behavior for raw PCM audio:

- RTP-like packet ordering and buffering
- adaptive target-delay management
- packet loss concealment and expansion
- accelerate/preemptive-expand time stretching
- runtime statistics

Packet payloads are expected to contain little-endian `f32` PCM samples. This copy does not include
encoded-audio decoding, browser bindings, auxiliary UI tooling, or native audio-player examples.
