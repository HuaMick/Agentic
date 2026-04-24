//! Story 18 acceptance test: the validation decision committed to by
//! the story — empty / whitespace-only values are rejected at every
//! tier with `SignerError::SignerInvalid`; any non-whitespace string
//! is accepted verbatim (no email-shape regex).
//!
//! Justification (from stories/18.yml acceptance.tests[4]):
//!   Proves the validation decision (the one open question
//!   note 10 asked story-writer to commit on): a signer
//!   value that is the empty string, or consists entirely
//!   of whitespace (` `, `\t`, `\n`, `\r`, or any mixture),
//!   is rejected at resolve time with
//!   `SignerError::SignerInvalid` naming the offending
//!   source (`flag`, `env`, or `git`). Non-whitespace
//!   strings of any shape — `"a"`, `"not-an-email"`,
//!   `"sandbox:claude-sonnet-4-6@run-a1b2c3"`,
//!   `"operator-17"` — are accepted verbatim.
//!   Specifically, no email-shape regex is applied: a
//!   signer is a free-form non-empty identity string. This
//!   pins the decision so a future contributor does not
//!   tighten the validator into an RFC-5321 gate that
//!   rejects the `sandbox:<model>@<run_id>` convention.
//!   Without this, either the validator is silently
//!   permissive (empty signers land in evidence rows) or
//!   silently restrictive (the sandbox convention fails
//!   validation the first time it's exercised).
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`, `SignerError`,
//! `Source`, `InvalidReason`).

use agentic_signer::{InvalidReason, Resolver, Signer, SignerError, Source};

#[test]
fn resolve_rejects_empty_and_whitespace_only_values_and_accepts_any_non_whitespace() {
    // The minimum rejection set from story 18's guidance.
    let rejected_values = [
        "",        // empty
        " ",       // single space
        "\t",      // single tab
        "\n",      // single newline
        "\r\n",    // CRLF
        " \t \n ", // mixed whitespace
    ];
    for value in rejected_values {
        let resolver = Resolver::with_flag(value);
        let err = Signer::resolve(resolver).expect_err(&format!(
            "whitespace-only flag {value:?} must be rejected, not silently accepted"
        ));
        match err {
            SignerError::SignerInvalid { source, reason } => {
                assert_eq!(
                    source,
                    Source::Flag,
                    "rejection must name Flag as the offending source for value {value:?}; got {source:?}"
                );
                // Reason must name Empty or WhitespaceOnly — either is
                // acceptable as long as a typed reason is attached.
                match reason {
                    InvalidReason::Empty | InvalidReason::WhitespaceOnly => {}
                }
            }
            other => panic!(
                "expected SignerError::SignerInvalid for value {value:?}; got {other:?} — \
                 silently permissive or silently tier-skipping is a regression"
            ),
        }
    }

    // The acceptance set. Each MUST resolve to a `Signer` carrying the
    // byte-identical value. Critically, the sandbox convention must
    // pass — any email-shape regex would reject it.
    let accepted_values = [
        "a",
        "not-an-email",
        "sandbox:claude-sonnet-4-6@run-a1b2c3",
        "operator-17",
    ];
    for value in accepted_values {
        let resolver = Resolver::with_flag(value);
        let signer = Signer::resolve(resolver).expect(&format!(
            "non-whitespace flag {value:?} must be accepted verbatim"
        ));
        assert_eq!(
            signer.as_str(),
            value,
            "accepted signer must be byte-identical to the input; got {:?} expected {value:?}",
            signer.as_str()
        );
    }
}
