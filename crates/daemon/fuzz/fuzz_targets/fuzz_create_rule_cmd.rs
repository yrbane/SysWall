//! Fuzz du converter gRPC CreateRuleRequest -> CreateRuleCommand (entrée non fiable).
//! Fuzzing of the gRPC CreateRuleRequest -> CreateRuleCommand converter (untrusted input).

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use syswall_daemon::grpc::converters::proto_to_create_rule_cmd;
use syswall_proto::syswall::CreateRuleRequest;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    name: String,
    priority: u32,
    criteria_json: String,
    scope_json: String,
    effect: String,
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
