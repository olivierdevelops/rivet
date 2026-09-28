//! Runtime target gate. Holds the effective Policy and applies the injected
//! decision function (the `policy.authorize_effect` use case, wired by the
//! orchestrator) to every actual attempt, recording each decision.

use crate::domain::policy::{EffectIntent, Permit, Policy};
use crate::domain::ports::PolicyEvaluator;
use std::sync::{Arc, Mutex};

pub type DecideFn = dyn Fn(&EffectIntent, &Policy, &str) -> Permit + Send + Sync;

// vhco:infra policy_broker satisfies PolicyEvaluator
pub struct PolicyBroker {
    policy: Policy,
    bundle_root: String,
    decide: Arc<DecideFn>,
    decisions: Mutex<Vec<Permit>>,
}

impl PolicyBroker {
    pub fn new(policy: Policy, bundle_root: &str, decide: Arc<DecideFn>) -> PolicyBroker {
        PolicyBroker {
            policy,
            bundle_root: bundle_root.to_string(),
            decide,
            decisions: Mutex::new(Vec::new()),
        }
    }

    /// Every decision taken so far (allow and deny), in order.
    pub fn decisions(&self) -> Vec<Permit> {
        self.decisions.lock().map(|d| d.clone()).unwrap_or_default()
    }
}

impl PolicyEvaluator for PolicyBroker {
    fn evaluate(&self, intent: &EffectIntent) -> Permit {
        let permit = (self.decide)(intent, &self.policy, &self.bundle_root);
        if let Ok(mut d) = self.decisions.lock() {
            if d.len() < 10_000 {
                d.push(permit.clone());
            }
        }
        permit
    }

    fn policy(&self) -> &Policy {
        &self.policy
    }
}
