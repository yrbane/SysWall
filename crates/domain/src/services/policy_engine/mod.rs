//! Policy engine: evaluate connections against rules to produce verdicts.
//! Moteur de politique : évalue les connexions par rapport aux règles pour produire des verdicts.

mod evaluator;
mod matcher;

use crate::entities::{ConnectionVerdict, RuleEffect, RuleId};
use crate::events::DefaultPolicy;

/// Result of evaluating a connection against rules.
/// Résultat de l'évaluation d'une connexion par rapport aux règles.
#[derive(Debug, Clone)]
pub struct PolicyEvaluation {
    pub verdict: ConnectionVerdict,
    pub matched_rule_id: Option<RuleId>,
    pub reason: EvaluationReason,
}

/// Why a particular verdict was reached.
/// Pourquoi un verdict particulier a été atteint.
#[derive(Debug, Clone)]
pub enum EvaluationReason {
    MatchedRule { rule_id: RuleId, effect: RuleEffect },
    NoMatchingRule,
    PendingUserDecision,
    DefaultPolicyApplied { policy: DefaultPolicy },
}

/// Pure domain service -- no I/O, no ports.
/// Service de domaine pur -- pas d'E/S, pas de ports.
pub struct PolicyEngine;
