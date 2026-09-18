# smysl feature requests from rust_smysl — for 1.7.0

**For:** the smysl 1.7.0 development branch. cargo-smysl pins the latest published release (1.6.0) and
adopts these when 1.7.0 ships.
**Previous:** R1–R9 (1.3.0), R10 and R12 (1.4.0), R16–R20 (1.5.0), R21–R23 (1.6.0). R11, R13 and R14 stay
open on purpose as S2 tasks.

| # | Request | Kind | Priority | Where it bites |
|---|---|---|---|---|
| R24 | Export `ExternalCost` and `CostModel` from the facade | Bug (API reach) | High | R21's `counting_with` cannot be called through `smysl` |

---

## R24 — `PackRequest::counting_with` cannot be reached through the facade

### What happens

1.6.0 added `PackRequest::counting_with(ExternalCost)` so a caller counting in its provider's tokens can
say so (R21, this project's request). `smysl-pack` exports `ExternalCost` and `CostModel`; the facade does
not:

```rust
pub use smysl_pack::{
    pack, verify as verify_pack, Constraints, Estimator, Pack, PackError, PackRequest, Reason,
    Selection, Violation,
};
```

A consumer that depends on `smysl` alone — which is what the pin rule and the self-contained objective
require — can name `PackRequest` and call `.counting_with(..)`, but cannot construct its argument.
Depending on `smysl-pack` directly would pin a second crate and bypass the facade's version contract.

### Why it matters here

Measured, and the reason this was found: `cargo smysl check` packs for a local model with a 16k window.
Under `Estimator::Utf8Div4` a 3 000-token pack is about 9 400 tokens to Qwen2.5-Coder, so prompts built to
fit 16k were 25k–57k. Ollama truncated them to the last 16 384 tokens — dropping the system prompt and the
units to judge — and said so only in its own server log. 43 of 63 held-out cases in one run were affected.

`counting_with` is the fix, and it is one `pub use` away from being usable.

### Acceptance

- `use smysl::{ExternalCost, PackRequest};` compiles, and
  `PackRequest::budget(b).counting_with(ExternalCost::new("id", f))` builds a pack whose `PackInfo::estimator`
  is `"id"`.
- `CostModel` is nameable through the facade for a caller that inspects `PackRequest::cost_model()`.
- `smysl::verify_pack` accepts a pack built under an external counter, as it does through `smysl-pack`.
