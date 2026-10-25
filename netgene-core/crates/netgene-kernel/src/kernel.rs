//! NetSphere Kernel — turns intents into policy-checked, audited actions.
//!
//! The LLM (or the rule-based fallback) proposes an action from the catalog;
//! the policy engine decides whether it runs now, waits for human
//! confirmation, or is denied; an [`Executor`] performs it and the result is
//! returned and audited.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use serde::Serialize;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agent::{AgentInfo, AgentStatus};
use crate::executor::{Executor, Workspace};
use netgene_gene::{crypto::GeneKeyPair, NetGene};
use netgene_llm::{LlmIntentEngine, OllamaClient};
use netgene_policy::{Action, AuditEntry, AuditLog, AuditRecord, Decision, Grant, Policy, PolicyEngine};
use crate::memory::KernelMemory;

/// Longest result kept in an audit record's detail.
const AUDIT_RESULT_CHARS: usize = 512;

/// Who the kernel acts as, which policy it enforces and where it audits.
pub struct KernelConfig {
    /// Gene on whose behalf intents are evaluated; its key signs the audit log.
    pub operator: NetGene,
    pub operator_kp: GeneKeyPair,
    pub policy: Policy,
    /// `None` keeps the audit log in memory.
    pub audit_path: Option<PathBuf>,
    /// Capabilities from verified capability tokens held by the operator.
    pub grants: Vec<Grant>,
    /// Enrolled hardware user-presence key (PKCS#1 RSA public key), if any.
    pub presence_key: Option<Vec<u8>>,
    /// Enrolled WebAuthn passkeys of the operator (another presence factor).
    pub passkeys: Vec<netgene_presence::webauthn::PasskeyCredential>,
    /// Where actions persist what they create.
    pub workspace: Workspace,
}

impl KernelConfig {
    /// Throw-away Master Gene, default policy, nothing on disk (tests, demos).
    pub fn ephemeral() -> Result<Self> {
        let (operator, operator_kp) = NetGene::generate_master("ephemeral-operator")?;
        Ok(Self {
            operator,
            operator_kp,
            policy: Policy::default(),
            audit_path: None,
            grants: vec![],
            presence_key: None,
            passkeys: vec![],
            workspace: Workspace::ephemeral()?,
        })
    }
}

/// Result of submitting an intent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DispatchOutcome {
    Executed { action: Action, explanation: String, result: serde_json::Value },
    PendingConfirmation { proposal_id: Uuid, action: Action, reason: String, explanation: String },
    Denied { action: Option<Action>, reason: String, explanation: String },
}

impl DispatchOutcome {
    /// One-line human summary.
    pub fn summary(&self) -> String {
        match self {
            DispatchOutcome::Executed { action, explanation, result } =>
                format!("✅ Executed: {}\n🧠 {}\n{}", action.kind, explanation, serde_json::to_string_pretty(result).unwrap_or_default()),
            DispatchOutcome::PendingConfirmation { proposal_id, action, reason, .. } =>
                format!("⏸️ Confirmation required for {} ({}). Proposal: {}", action.kind, reason, proposal_id),
            DispatchOutcome::Denied { reason, .. } => format!("⛔ Denied by policy: {}", reason),
        }
    }
}

/// The Netsphere Kernel state.
pub struct NetSphereKernel {
    /// Kernel memory.
    pub memory: Arc<Mutex<KernelMemory>>,
    operator: NetGene,
    policy: PolicyEngine,
    audit: Mutex<AuditLog>,
    /// Actions waiting for human confirmation: id → (original input, action).
    pending: Mutex<HashMap<Uuid, (String, Action)>>,
    grants: Vec<Grant>,
    presence_key: Option<Vec<u8>>,
    passkeys: Mutex<Vec<netgene_presence::webauthn::PasskeyCredential>>,
    executor: Mutex<Executor>,
    /// Marketplace manifests installed through [`Self::inject_marketplace_agent`].
    marketplace_agents: Mutex<Vec<AgentInfo>>,
    booted_at: chrono::DateTime<chrono::Utc>,
}

impl NetSphereKernel {
    /// Boot with an ephemeral operator and the default policy.
    pub async fn boot() -> Result<Self> {
        Self::boot_with(KernelConfig::ephemeral()?).await
    }

    pub async fn boot_with(config: KernelConfig) -> Result<Self> {
        let executor_kp = config.operator_kp.try_clone()?;
        let audit = match config.audit_path {
            Some(path) => AuditLog::open(path, config.operator_kp)?,
            None => AuditLog::in_memory(config.operator_kp),
        };
        info!("✅ Netsphere Kernel online as {} ({})", config.operator.name, config.operator.short_fp);

        Ok(Self {
            memory: Arc::new(Mutex::new(KernelMemory::new())),
            executor: Mutex::new(Executor::new(config.workspace, config.operator.clone(), executor_kp)),
            operator: config.operator,
            policy: PolicyEngine::new(config.policy),
            audit: Mutex::new(audit),
            pending: Mutex::new(HashMap::new()),
            grants: config.grants,
            presence_key: config.presence_key,
            passkeys: Mutex::new(config.passkeys),
            marketplace_agents: Mutex::new(vec![]),
            booted_at: chrono::Utc::now(),
        })
    }

    /// Submit a natural-language intent. The LLM (or the rule-based fallback)
    /// only *proposes* an action; the policy engine decides whether it runs
    /// now, waits for confirmation, or is denied. Every step is audited.
    pub async fn propose_intent(&self, input: &str) -> Result<DispatchOutcome> {
        let client = OllamaClient::new(Some("http://localhost:11434"), Some("llama3"));
        let engine = LlmIntentEngine::new(client);

        let llm_intent = engine.parse(input).await.unwrap_or_else(|e| {
            warn!("LLM parse failed: {}", e);
            netgene_llm::LlmParsedIntent {
                action: "unknown".to_string(),
                parameters: serde_json::json!({}),
                explanation: format!("LLM error: {}", e),
            }
        });
        let explanation = llm_intent.explanation.clone();

        {
            let mut mem = self.memory.lock().await;
            mem.log_event(format!("Intent: {} (proposed: {})", input, llm_intent.action));
        }

        let Some(action) = Action::from_llm_intent(&llm_intent.action, llm_intent.parameters.clone()) else {
            let reason = format!("'{}' is not an executable action", llm_intent.action);
            self.audit(input, None, "deny", &reason).await?;
            return Ok(DispatchOutcome::Denied { action: None, reason, explanation });
        };
        self.propose_action(input, action, explanation).await
    }

    /// Submit an already structured action (CLI, API) through the same policy path.
    pub async fn propose_action(&self, input: &str, action: Action, explanation: String) -> Result<DispatchOutcome> {
        match self.evaluate(&action) {
            Decision::Allow => {
                self.audit(input, Some(&action), "allow", "").await?;
                self.execute(input, action, explanation).await
            }
            Decision::Confirm { reason } => {
                self.audit(input, Some(&action), "confirm", &reason).await?;
                let proposal_id = Uuid::new_v4();
                self.pending.lock().await.insert(proposal_id, (input.to_string(), action.clone()));
                Ok(DispatchOutcome::PendingConfirmation { proposal_id, action, reason, explanation })
            }
            Decision::Deny { reason } => {
                self.audit(input, Some(&action), "deny", &reason).await?;
                Ok(DispatchOutcome::Denied { action: Some(action), reason, explanation })
            }
        }
    }

    /// Whether confirmations need a hardware user-presence proof.
    pub fn requires_user_presence(&self) -> bool {
        self.policy.policy().require_user_presence
    }

    /// Bytes the operator's presence key must sign to approve `proposal_id`.
    pub async fn presence_challenge(&self, proposal_id: Uuid) -> Result<Vec<u8>> {
        let pending = self.pending.lock().await;
        let (_, action) = pending
            .get(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;
        netgene_presence::challenge(proposal_id, &action.kind, &action.params)
    }

    /// Human approval of a pending proposal (when the policy does not require
    /// a user-presence proof).
    pub async fn confirm(&self, proposal_id: Uuid) -> Result<DispatchOutcome> {
        if self.requires_user_presence() {
            anyhow::bail!("Policy requires a user-presence proof to confirm (use confirm_with_presence)");
        }
        self.confirm_inner(proposal_id, format!("proposal {}", proposal_id)).await
    }

    /// Human approval backed by a hardware user-presence signature over
    /// [`Self::presence_challenge`].
    pub async fn confirm_with_presence(&self, proposal_id: Uuid, signature: &[u8]) -> Result<DispatchOutcome> {
        let key = self
            .presence_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No user-presence key enrolled for the operator"))?;
        let challenge = self.presence_challenge(proposal_id).await?;
        if !netgene_presence::verify(key, &challenge, signature) {
            let pending = self.pending.lock().await;
            if let Some((input, action)) = pending.get(&proposal_id) {
                self.audit(input, Some(action), "presence_failed", &format!("proposal {}", proposal_id)).await?;
            }
            anyhow::bail!("User-presence signature is invalid for proposal {}", proposal_id);
        }
        self.confirm_inner(proposal_id, format!("proposal {} (user presence verified)", proposal_id)).await
    }

    /// `navigator.credentials.get` options whose challenge binds `proposal_id`.
    pub async fn passkey_request(&self, proposal_id: Uuid, rp_id: &str) -> Result<serde_json::Value> {
        let passkeys = self.passkeys.lock().await;
        if passkeys.is_empty() {
            anyhow::bail!("No passkey enrolled for the operator");
        }
        let challenge = self.presence_challenge(proposal_id).await?;
        Ok(netgene_presence::webauthn::request_options(&passkeys, &challenge, rp_id))
    }

    /// Human approval backed by a WebAuthn assertion over the proposal's
    /// challenge, made on `origin`. Returns the outcome and the passkeys with
    /// their updated signature counters (to persist).
    pub async fn confirm_with_passkey(
        &self,
        proposal_id: Uuid,
        assertion: &netgene_presence::webauthn::AssertionResponse,
        origin: &str,
    ) -> Result<(DispatchOutcome, Vec<netgene_presence::webauthn::PasskeyCredential>)> {
        let challenge = self.presence_challenge(proposal_id).await?;
        let verified = {
            let passkeys = self.passkeys.lock().await;
            netgene_presence::webauthn::verify_assertion(&passkeys, assertion, &challenge, origin)
        };
        let (index, count) = match verified {
            Ok(v) => v,
            Err(e) => {
                let pending = self.pending.lock().await;
                if let Some((input, action)) = pending.get(&proposal_id) {
                    self.audit(input, Some(action), "presence_failed", &format!("proposal {} (passkey: {})", proposal_id, e)).await?;
                }
                return Err(e.context(format!("Passkey approval refused for proposal {}", proposal_id)));
            }
        };
        let updated = {
            let mut passkeys = self.passkeys.lock().await;
            passkeys[index].sign_count = count;
            passkeys.clone()
        };
        let outcome = self.confirm_inner(proposal_id, format!("proposal {} (passkey verified)", proposal_id)).await?;
        Ok((outcome, updated))
    }

    async fn confirm_inner(&self, proposal_id: Uuid, detail: String) -> Result<DispatchOutcome> {
        let (input, action) = self
            .pending
            .lock()
            .await
            .remove(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;

        // Re-check: the operator could have been suspended since the proposal.
        if let Decision::Deny { reason } = self.evaluate(&action) {
            self.audit(&input, Some(&action), "deny", &reason).await?;
            return Ok(DispatchOutcome::Denied { action: Some(action), reason, explanation: String::new() });
        }

        self.audit(&input, Some(&action), "confirmed", &detail).await?;
        self.execute(&input, action, "confirmed by operator".to_string()).await
    }

    /// Discard a pending proposal.
    pub async fn reject(&self, proposal_id: Uuid) -> Result<()> {
        let (input, action) = self
            .pending
            .lock()
            .await
            .remove(&proposal_id)
            .ok_or_else(|| anyhow::anyhow!("No pending proposal {}", proposal_id))?;
        self.audit(&input, Some(&action), "rejected", &format!("proposal {}", proposal_id)).await?;
        Ok(())
    }

    /// Policy decision for the operator, counting only unexpired grants.
    fn evaluate(&self, action: &Action) -> Decision {
        let now = chrono::Utc::now();
        let granted: Vec<String> = self
            .grants
            .iter()
            .filter(|g| g.is_active(now))
            .flat_map(|g| g.capabilities.iter().cloned())
            .collect();
        self.policy.evaluate_with_grants(&self.operator, &granted, action)
    }

    /// Compatibility wrapper returning a human-readable summary.
    pub async fn dispatch_intent(&self, input: &str) -> Result<String> {
        Ok(self.propose_intent(input).await?.summary())
    }

    /// Entries of the audit log.
    pub async fn audit_entries(&self) -> Result<Vec<AuditEntry>> {
        self.audit.lock().await.entries()
    }

    /// The Gene the kernel acts as.
    pub fn operator(&self) -> &NetGene {
        &self.operator
    }

    /// The policy currently enforced.
    pub fn policy(&self) -> &Policy {
        self.policy.policy()
    }

    /// Feed a metric sample (desktop telemetry, eBPF counters) to the anomaly detector.
    pub async fn observe_metric(&self, metric: &str, value: f64) -> Option<netgene_safeguard::AnomalyEvent> {
        self.executor.lock().await.observe(metric, value)
    }

    async fn audit(&self, input: &str, action: Option<&Action>, decision: &str, detail: &str) -> Result<()> {
        self.audit.lock().await.append(AuditRecord {
            actor_gene_id: self.operator.id.to_string(),
            input: input.to_string(),
            action: action.map(|a| a.kind.clone()),
            params: action.map(|a| a.params.clone()).unwrap_or(serde_json::Value::Null),
            decision: decision.to_string(),
            detail: detail.to_string(),
        })?;
        Ok(())
    }

    /// Run an approved catalog action; failures are audited and reported.
    async fn execute(&self, input: &str, action: Action, explanation: String) -> Result<DispatchOutcome> {
        let result = {
            let mut executor = self.executor.lock().await;
            match action.kind.as_str() {
                "system.status" => executor.status(serde_json::json!({
                    "uptime_seconds": (chrono::Utc::now() - self.booted_at).num_seconds(),
                    "pending_proposals": self.pending.lock().await.len(),
                    "audit_entries": self.audit.lock().await.entries()?.len(),
                    "require_user_presence": self.requires_user_presence(),
                })),
                "safeguard.scan" => executor.scan().await,
                "node.provision" => executor.provision(&action.params),
                "agent.list" => executor.list_agents(),
                "agent.run" => executor.run_agent(&action.params, &self.policy.policy().agent_permissions).await,
                other => Err(anyhow::anyhow!("No executor for action '{}'", other)),
            }
        };
        match result {
            Ok(result) => {
                let mut summary = result.to_string();
                if summary.chars().count() > AUDIT_RESULT_CHARS {
                    summary = summary.chars().take(AUDIT_RESULT_CHARS).collect::<String>() + "…";
                }
                self.audit(input, Some(&action), "executed", &summary).await?;
                self.memory.lock().await.log_event(format!("Executed {}", action.kind));
                Ok(DispatchOutcome::Executed { action, explanation, result })
            }
            Err(e) => {
                self.audit(input, Some(&action), "failed", &e.to_string()).await?;
                Err(e.context(format!("{} failed", action.kind)))
            }
        }
    }

    pub async fn inject_marketplace_agent(&self, id: String, name: String) {
        self.marketplace_agents.lock().await.push(AgentInfo {
            id: Uuid::new_v4(),
            name,
            agent_type: id,
            status: AgentStatus::Idle,
            messages_processed: 0,
            started_at: chrono::Utc::now(),
        });
    }

    /// Installed WASM agents and marketplace manifests.
    pub async fn agent_list(&self) -> Vec<AgentInfo> {
        let mut agents = self.marketplace_agents.lock().await.clone();
        let listed = self.executor.lock().await.list_agents().unwrap_or_default();
        for agent in listed["agents"].as_array().into_iter().flatten() {
            agents.push(AgentInfo {
                id: Uuid::new_v5(&Uuid::NAMESPACE_OID, agent["name"].as_str().unwrap_or_default().as_bytes()),
                name: agent["name"].as_str().unwrap_or_default().to_string(),
                agent_type: "wasm".into(),
                status: if agent["trusted"].as_bool() == Some(true) { AgentStatus::Idle } else { AgentStatus::Error("untrusted author".into()) },
                messages_processed: 0,
                started_at: self.booted_at,
            });
        }
        agents
    }

    /// Install a signed WASM agent authored by the operator's lineage.
    pub async fn install_wasm_agent(&self, module: &netgene_wasm::GeneModule) -> Result<PathBuf> {
        let executor = self.executor.lock().await;
        crate::executor::install_agent(&executor.workspace, &executor.trusted_authors()?, module)
    }

    /// Graceful shutdown (nothing runs in the background any more).
    pub async fn shutdown(&self) -> Result<()> {
        info!("Netsphere Kernel shutting down...");
        Ok(())
    }
}
