//! Fuzz du parsing JSON des critères et scopes de règles (entrées gRPC non fiables).
//! Fuzzing of rule criteria/scope JSON parsing (untrusted gRPC inputs).

#![no_main]

use libfuzzer_sys::fuzz_target;
use syswall_domain::entities::{Rule, RuleCriteria, RuleScope};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<RuleCriteria>(s);
        let _ = serde_json::from_str::<RuleScope>(s);
        let _ = serde_json::from_str::<Rule>(s);
    }
});
