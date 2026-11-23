# Basalt runtime issues

- **Live GPU availability**. The compatible GPU was occupied during implementation. The separate workload had stopped by the time verification began. Live verification used the available GPU and confirmed that unloading Basalt released its memory.
- **Model acquisition**. Pumas downloaded the 19.7 GB DQ8 pack and published it after validation. The file matched the publisher digest.
- **Reasoning-only output**. Basalt can use the entire output limit before emitting answer text, and it sends reasoning events separately in streams. The finite typed operation preserves the truncation result, and the stream publishes answer text and completion without forwarding reasoning chunks. Both paths passed with the real pack.
