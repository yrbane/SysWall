//! Fuzz du converter gRPC CreateRuleRequest -> CreateRuleCommand (entrée non fiable).
//! Fuzzing of the gRPC CreateRuleRequest -> CreateRuleCommand converter (untrusted input).

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use syswall_daemon::grpc::converters::proto_to_create_rule_cmd;
use syswall_proto::syswall::CreateRuleRequest;

/// Biaise effect vers les valeurs valides tout en gardant des entrées arbitraires (4/5 valid, 1/5 garbage).
/// Biases effect toward valid values while still producing arbitrary inputs (4/5 valid, 1/5 garbage).
fn arbitrary_effect(u: &mut Unstructured) -> arbitrary::Result<String> {
    // Valeurs valides pour parse_rule_effect (pas de "defer:300" — réservé aux décisions).
    // Valid values for parse_rule_effect ("defer:300" is for decisions, not rule effects).
    let choices = ["allow", "block", "ask", "observe", ""];
    if u.ratio(4u8, 5u8)? {
        Ok((*u.choose(&choices)?).to_string())
    } else {
        String::arbitrary(u)
    }
}

/// Biaise source vers les valeurs valides tout en gardant des entrées arbitraires (4/5 valid, 1/5 garbage).
/// Biases source toward valid values while still producing arbitrary inputs (4/5 valid, 1/5 garbage).
fn arbitrary_source(u: &mut Unstructured) -> arbitrary::Result<String> {
    // Valeurs valides pour parse_rule_source.
    // Valid values for parse_rule_source.
    let choices = ["manual", "auto_learning", "import", "system", ""];
    if u.ratio(4u8, 5u8)? {
        Ok((*u.choose(&choices)?).to_string())
    } else {
        String::arbitrary(u)
    }
}

/// Biaise criteria_json vers des structures JSON valides (50% valid, 50% garbage).
/// Biases criteria_json toward valid JSON structures (50% valid, 50% garbage).
fn arbitrary_criteria_json(u: &mut Unstructured) -> arbitrary::Result<String> {
    let valid = [
        // Critères vides : tous les champs None (correspond à tout).
        // Empty criteria: all fields None (matches everything).
        r#"{"application":null,"user":null,"remote_ip":null,"remote_port":null,"local_port":null,"protocol":null,"direction":null,"schedule":null}"#,
        // Critères complets : application par nom, IP exacte, port HTTPS, TCP sortant.
        // Full criteria: application by name, exact IP, HTTPS port, outbound TCP.
        r#"{"application":{"ByName":"firefox"},"user":null,"remote_ip":{"Exact":"93.184.216.34"},"remote_port":{"Exact":443},"local_port":null,"protocol":"Tcp","direction":"Outbound","schedule":null}"#,
        // Critères CIDR : réseau entier plutôt qu'IP exacte.
        // CIDR criteria: entire network range rather than exact IP.
        r#"{"application":{"ByName":"firefox"},"user":null,"remote_ip":{"Cidr":{"network":"10.0.0.0","prefix_len":8}},"remote_port":{"Exact":443},"local_port":null,"protocol":"Tcp","direction":"Outbound","schedule":null}"#,
    ];
    if u.ratio(1u8, 2u8)? {
        Ok((*u.choose(&valid)?).to_string())
    } else {
        String::arbitrary(u)
    }
}

/// Biaise scope_json vers des structures JSON valides (50% valid, 50% garbage).
/// Biases scope_json toward valid JSON structures (50% valid, 50% garbage).
fn arbitrary_scope_json(u: &mut Unstructured) -> arbitrary::Result<String> {
    // Formes valides de RuleScope sérialisées en JSON.
    // Valid serialized forms of RuleScope.
    let valid = [
        r#""Permanent""#,
        r#"{"Temporary":{"expires_at":"2026-06-11T12:00:00Z"}}"#,
    ];
    if u.ratio(1u8, 2u8)? {
        Ok((*u.choose(&valid)?).to_string())
    } else {
        String::arbitrary(u)
    }
}

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    name: String,
    priority: u32,
    #[arbitrary(with = arbitrary_criteria_json)]
    criteria_json: String,
    #[arbitrary(with = arbitrary_scope_json)]
    scope_json: String,
    #[arbitrary(with = arbitrary_effect)]
    effect: String,
    #[arbitrary(with = arbitrary_source)]
    source: String,
}

fuzz_target!(|input: FuzzInput| {
    let req = CreateRuleRequest {
        name: input.name,
        priority: input.priority,
        criteria_json: input.criteria_json,
        scope_json: input.scope_json,
        effect: input.effect,
        source: input.source,
    };
    let _ = proto_to_create_rule_cmd(&req);
});
