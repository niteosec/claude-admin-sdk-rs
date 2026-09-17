//! Known-answer signature tests.
//!
//! Expected signatures were computed independently with Python's `hmac`, `hashlib` and `base64`,
//! following the Python sample on the "Develop an Inference hooks integration" page:
//! `b"v1," + base64.b64encode(hmac.new(base64.b64decode(secret.removeprefix("whsec_"), validate=True),
//! f"{id}.{ts}.".encode() + body, hashlib.sha256).digest())`, with `body` = the bytes of
//! `fixtures/docs/prompt_frame.json`.

use std::time::{Duration, UNIX_EPOCH};

use claude_inference_hooks::{
    HookRequest, InvalidSecretError, ReceiveError, SigningSecret, Verifier, VerifyError, WEBHOOK_SIGNATURE,
};
use http::{HeaderMap, HeaderName, HeaderValue};

const BODY: &[u8] = include_bytes!("fixtures/docs/prompt_frame.json");

// Every secret contains both `+` and `/`.
const SECRET_A: &str = "whsec_ZcXPRJHeWXWU/5BqC2Al267GeO3KD3ooa4jBlVj4+e8=";
const SECRET_B: &str = "whsec_+q9RXghbk8Mk1HlXOugkdkj/b1BWMzgQvqJ91m9WO1I=";
const SECRET_C: &str = "whsec_VTBacdgT/CFGgwmdlMgUpl0uMBvwVmAYaM+UF6nLglU=";

const ID: &str = "req_abc123";
const TS: i64 = 1_789_000_000;
const SIG_A: &str = "v1,xnEkMUWU41TKdRKxZmM1qLLVA0PcZEyd22PrXT/aRPU=";
const SIG_B: &str = "v1,uFcd14ht1Q5QPfVNb4u4hA+YlJHBLd4CFGeNO2JXv0Y=";
const SIG_C: &str = "v1,PbI6H1M0dpYDS0CsGY1BG+eB6BDo3kS36wkUpON30ew=";
/// Secret A, `webhook-id` = `req_other`, same timestamp and body.
const SIG_A_OTHER_ID: &str = "v1,25fTRONKwrV1ugSK/IjtSfX9RDPq0HTNGShae0eB4Bo=";
/// Secret A, timestamp 1788999000.
const SIG_A_OLD: &str = "v1,XcAy+oUY7WuAJ4GFVh5aijNbndeH/zmqRHYTqiGg5FY=";
/// Secret A, timestamp 1789001000.
const SIG_A_FUTURE: &str = "v1,3z/6UucFUZblD7MZWcmOnc/Vuxww5S45OMaRrPpeDVw=";
/// Secret A, `webhook-id` = `msg_empty`, empty body.
const SIG_A_EMPTY_BODY: &str = "v1,Xv0BwfRGbY6qxXph9yz5+K3dLUca1vGppMrwfB7/eAo=";

fn secret(value: &str) -> SigningSecret {
    SigningSecret::parse(value).unwrap()
}

fn headers(id: &str, ts: &str, signature: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("webhook-id", HeaderValue::from_str(id).unwrap());
    headers.insert("webhook-timestamp", HeaderValue::from_str(ts).unwrap());
    headers.insert("webhook-signature", HeaderValue::from_str(signature).unwrap());
    headers
}

#[test]
fn sign_reproduces_the_python_vectors() {
    let ts = TS.to_string();
    assert_eq!(secret(SECRET_A).sign(ID, &ts, BODY), SIG_A);
    assert_eq!(secret(SECRET_B).sign(ID, &ts, BODY), SIG_B);
    assert_eq!(secret(SECRET_C).sign(ID, &ts, BODY), SIG_C);
    assert_eq!(secret(SECRET_A).sign("req_other", &ts, BODY), SIG_A_OTHER_ID);
    assert_eq!(secret(SECRET_A).sign(ID, "1788999000", BODY), SIG_A_OLD);
    assert_eq!(secret(SECRET_A).sign(ID, "1789001000", BODY), SIG_A_FUTURE);
    assert_eq!(secret(SECRET_A).sign("msg_empty", &ts, b""), SIG_A_EMPTY_BODY);
}

#[test]
fn valid_signature_verifies_and_parses() {
    let verifier = Verifier::new(secret(SECRET_A));
    let verified = verifier.verify_request_at(&headers(ID, "1789000000", SIG_A), BODY, TS).unwrap();
    assert_eq!(verified.delivery.webhook_id, ID);
    assert_eq!(verified.delivery.timestamp, TS);
    assert!(matches!(verified.request, HookRequest::Prompt(ref frame) if frame.request_id == ID));

    let empty = verifier.verify_at(&headers("msg_empty", "1789000000", SIG_A_EMPTY_BODY), b"", TS).unwrap();
    assert_eq!(empty.webhook_id, "msg_empty");
}

#[test]
fn system_time_entry_point_matches_unix_seconds() {
    let verifier = Verifier::new(secret(SECRET_A));
    let now = UNIX_EPOCH + Duration::from_secs(TS as u64);
    assert!(verifier.verify(&headers(ID, "1789000000", SIG_A), BODY, now).is_ok());
    assert!(verifier.verify_request(&headers(ID, "1789000000", SIG_A), BODY, now).is_ok());
}

#[test]
fn header_names_are_case_insensitive() {
    let mut map = HeaderMap::new();
    map.insert(HeaderName::from_bytes(b"Webhook-Id").unwrap(), HeaderValue::from_static(ID));
    map.insert(HeaderName::from_bytes(b"WEBHOOK-TIMESTAMP").unwrap(), HeaderValue::from_static("1789000000"));
    map.insert(HeaderName::from_bytes(b"Webhook-Signature").unwrap(), HeaderValue::from_static(SIG_A));
    assert!(Verifier::new(secret(SECRET_A)).verify_at(&map, BODY, TS).is_ok());
}

#[test]
fn any_matching_value_among_several_is_accepted() {
    let verifier = Verifier::new(secret(SECRET_A));
    for signature in [
        format!("{SIG_C} {SIG_A}"),
        format!("{SIG_A} {SIG_B}"),
        format!("v1a,somethingelse {SIG_B}  {SIG_A}"),
        format!("v1,notbase64!! {SIG_A}"),
    ] {
        assert!(verifier.verify_at(&headers(ID, "1789000000", &signature), BODY, TS).is_ok(), "{signature}");
    }
}

#[test]
fn rotation_accepts_either_secret() {
    let verifier = Verifier::new(secret(SECRET_B)).with_secret(secret(SECRET_A));
    assert!(verifier.verify_at(&headers(ID, "1789000000", SIG_A), BODY, TS).is_ok());
    assert!(verifier.verify_at(&headers(ID, "1789000000", SIG_B), BODY, TS).is_ok());
    assert_eq!(
        verifier.verify_at(&headers(ID, "1789000000", SIG_C), BODY, TS).unwrap_err(),
        VerifyError::NoMatchingSignature
    );
}

#[test]
fn tampered_body_is_rejected() {
    let mut tampered = BODY.to_vec();
    let at = tampered.windows(3).position(|w| w == b"14%").unwrap();
    tampered[at + 1] = b'5';
    assert_eq!(
        Verifier::new(secret(SECRET_A)).verify_at(&headers(ID, "1789000000", SIG_A), &tampered, TS).unwrap_err(),
        VerifyError::NoMatchingSignature
    );
    // Re-encoding the JSON also breaks the signature: verify raw bytes.
    let reencoded = serde_json::to_vec(&serde_json::from_slice::<serde_json::Value>(BODY).unwrap()).unwrap();
    assert!(Verifier::new(secret(SECRET_A)).verify_at(&headers(ID, "1789000000", SIG_A), &reencoded, TS).is_err());
}

#[test]
fn wrong_secret_or_signed_fields_are_rejected() {
    let verifier = Verifier::new(secret(SECRET_B));
    assert_eq!(
        verifier.verify_at(&headers(ID, "1789000000", SIG_A), BODY, TS).unwrap_err(),
        VerifyError::NoMatchingSignature
    );
    let verifier = Verifier::new(secret(SECRET_A));
    assert_eq!(
        verifier.verify_at(&headers("req_other", "1789000000", SIG_A), BODY, TS).unwrap_err(),
        VerifyError::NoMatchingSignature
    );
}

#[test]
fn secret_must_use_the_standard_alphabet() {
    let url_safe = SECRET_A.replace('+', "-").replace('/', "_");
    assert_eq!(SigningSecret::parse(&url_safe).unwrap_err(), InvalidSecretError::NotStandardBase64);
    // And a signature in the URL-safe alphabet does not match either.
    let url_safe_sig = SIG_A.replace('+', "-").replace('/', "_");
    assert_eq!(
        Verifier::new(secret(SECRET_A)).verify_at(&headers(ID, "1789000000", &url_safe_sig), BODY, TS).unwrap_err(),
        VerifyError::MalformedSignature
    );
    assert_eq!(SigningSecret::parse("whsec_not base64").unwrap_err(), InvalidSecretError::NotStandardBase64);
}

#[test]
fn timestamps_outside_five_minutes_are_rejected() {
    let verifier = Verifier::new(secret(SECRET_A));
    let old = headers(ID, "1788999000", SIG_A_OLD);
    let future = headers(ID, "1789001000", SIG_A_FUTURE);
    assert_eq!(
        verifier.verify_at(&old, BODY, TS).unwrap_err(),
        VerifyError::TimestampTooOld { timestamp: 1_788_999_000, now: TS }
    );
    assert_eq!(
        verifier.verify_at(&future, BODY, TS).unwrap_err(),
        VerifyError::TimestampTooNew { timestamp: 1_789_001_000, now: TS }
    );
    // The same signatures verify when the clock agrees.
    assert!(verifier.verify_at(&old, BODY, 1_788_999_000).is_ok());
    assert!(verifier.verify_at(&future, BODY, 1_789_001_000).is_ok());

    // Boundary: exactly 300 s either way is accepted, 301 s is not.
    let current = headers(ID, "1789000000", SIG_A);
    assert!(verifier.verify_at(&current, BODY, TS + 300).is_ok());
    assert!(verifier.verify_at(&current, BODY, TS - 300).is_ok());
    assert!(matches!(verifier.verify_at(&current, BODY, TS + 301), Err(VerifyError::TimestampTooOld { .. })));
    assert!(matches!(verifier.verify_at(&current, BODY, TS - 301), Err(VerifyError::TimestampTooNew { .. })));

    // Extreme values do not overflow.
    assert!(verifier.verify_at(&current, BODY, i64::MIN).is_err());
    assert!(verifier.verify_at(&headers(ID, &i64::MIN.to_string(), SIG_A), BODY, i64::MAX).is_err());

    // A custom tolerance applies.
    assert!(verifier.clone().with_tolerance_seconds(1000).verify_at(&old, BODY, TS).is_ok());
}

#[test]
fn missing_and_malformed_headers() {
    let verifier = Verifier::new(secret(SECRET_A));
    assert_eq!(verifier.verify_at(&HeaderMap::new(), BODY, TS).unwrap_err(), VerifyError::Unsigned);

    for name in ["webhook-id", "webhook-timestamp", "webhook-signature"] {
        let mut map = headers(ID, "1789000000", SIG_A);
        map.remove(name);
        assert!(matches!(verifier.verify_at(&map, BODY, TS), Err(VerifyError::MissingHeader(n)) if n == name));
        let mut map = headers(ID, "1789000000", SIG_A);
        map.insert(HeaderName::from_static(name), HeaderValue::from_static(""));
        assert!(matches!(verifier.verify_at(&map, BODY, TS), Err(VerifyError::MissingHeader(n)) if n == name));
    }

    let mut map = headers(ID, "1789000000", SIG_A);
    map.insert("webhook-id", HeaderValue::from_bytes(b"req_\xff").unwrap());
    assert_eq!(verifier.verify_at(&map, BODY, TS).unwrap_err(), VerifyError::MalformedHeader("webhook-id"));

    for ts in ["abc", "1789000000.5", "1e9"] {
        assert_eq!(
            verifier.verify_at(&headers(ID, ts, SIG_A), BODY, TS).unwrap_err(),
            VerifyError::MalformedTimestamp,
            "{ts}"
        );
    }

    for signature in ["garbage", "v1,", "v1,AAAA", "v2,xnEkMUWU41TKdRKxZmM1qLLVA0PcZEyd22PrXT/aRPU=", "   "] {
        let mut map = headers(ID, "1789000000", SIG_A);
        map.insert(WEBHOOK_SIGNATURE, HeaderValue::from_static(signature));
        let error = verifier.verify_at(&map, BODY, TS).unwrap_err();
        let expected = if signature.trim().is_empty() {
            VerifyError::MissingHeader("webhook-signature")
        } else {
            VerifyError::MalformedSignature
        };
        assert_eq!(error, expected, "{signature}");
    }
}

#[test]
fn webhook_id_must_equal_request_id() {
    let verifier = Verifier::new(secret(SECRET_A));
    // The signature is valid for `req_other`, but the body says `req_abc123`.
    let map = headers("req_other", "1789000000", SIG_A_OTHER_ID);
    assert!(verifier.verify_at(&map, BODY, TS).is_ok());
    match verifier.verify_request_at(&map, BODY, TS).unwrap_err() {
        ReceiveError::WebhookIdMismatch { webhook_id, request_id } => {
            assert_eq!((webhook_id.as_str(), request_id.as_str()), ("req_other", ID));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn signed_but_invalid_body_is_a_body_error() {
    let secret = secret(SECRET_A);
    let body = b"not json";
    let map = headers(ID, "1789000000", &secret.sign(ID, "1789000000", body));
    assert!(matches!(Verifier::new(secret).verify_request_at(&map, body, TS), Err(ReceiveError::Body(_))));
}
