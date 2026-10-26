//! Gene Layer CLI commands.

use clap::Subcommand;
use anyhow::Result;
use super::grants;
use netgene_gene::{
    identity::NetGene,
    CapabilityToken, RevocationList,
    storage::{default_gene_dir, save_gene, list_genes, load_gene},
};

#[derive(Subcommand)]
pub enum GeneCommand {
    /// Generate a new Master Gene (root identity)
    Init {
        /// Gene name (e.g., your name or node name)
        #[arg(short, long, default_value = "NetGene Master")]
        name: String,
    },
    /// Show all stored genes
    Show,
    /// Spawn a Sub-Gene from the master
    Spawn {
        /// Parent gene ID
        #[arg(long)]
        parent: String,
        /// New gene name
        #[arg(short, long)]
        name: String,
        /// Role: node | agent | observer
        #[arg(short, long, default_value = "node")]
        role: String,
    },
    /// Verify a gene by ID
    Verify {
        /// Gene ID to verify
        id: String,
    },
    /// Print a gene's public identity (JSON) to share with other nodes
    Export {
        /// Gene ID
        id: String,
    },
    /// Import another node's public identity (JSON produced by `gene export`)
    Import {
        /// Path to the exported JSON
        file: std::path::PathBuf,
    },
    /// Grant capabilities to a child gene with a time-limited, signed token
    Grant {
        /// Issuing gene (must be the subject's parent)
        #[arg(long)]
        issuer: String,
        /// Gene receiving the capabilities
        #[arg(long)]
        subject: String,
        /// Capabilities, comma separated (e.g. node.spawn,network.admin)
        #[arg(long, value_delimiter = ',', required = true)]
        caps: Vec<String>,
        /// Lifetime in minutes
        #[arg(long, default_value_t = 60)]
        ttl_minutes: i64,
    },
    /// List capability tokens and whether each is currently valid
    Tokens,
    /// Revoke a capability token by id
    RevokeToken {
        token_id: uuid::Uuid,
    },
    /// Create a hardware user-presence key (Windows Hello) for confirming actions
    EnrollPresence {
        #[arg(long)]
        gene: String,
    },
    /// Enrol a WebAuthn passkey (this device or a phone) as a presence factor
    EnrollPasskey {
        #[arg(long)]
        gene: String,
    },
    /// Grant capabilities from a SAML 2.0 response (mapping in ~/.netgene/saml.json)
    #[cfg(feature = "saml")]
    GrantSaml {
        /// Issuing gene (the subject's parent, normally the Master Gene)
        #[arg(long)]
        issuer: String,
        /// Gene that receives the capabilities
        #[arg(long)]
        subject: String,
        /// Base64 SAMLResponse (HTTP-POST binding), or @path to a file containing it
        #[arg(long)]
        response: String,
    },
    /// Grant capabilities from a corporate OIDC ID token (mapping in ~/.netgene/enterprise.json)
    GrantOidc {
        /// Issuing gene (the subject's parent, normally the Master Gene)
        #[arg(long)]
        issuer: String,
        /// Gene that receives the capabilities
        #[arg(long)]
        subject: String,
        /// The ID token (JWT), or @path to a file containing it
        #[arg(long)]
        id_token: String,
    },
}

pub async fn run(cmd: GeneCommand, json: bool) -> Result<()> {
    let dir = default_gene_dir();
    let secrets = netgene_gene::default_store()?;

    match cmd {
        GeneCommand::Init { name } => {
            if !json {
                println!("🧬 Generating Master Gene for '{}'...", name);
            }
            let (gene, kp) = NetGene::generate_master(&name)?;
            save_gene(&gene, &kp, &dir, &secrets)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&gene)?);
            } else {
                println!();
                println!("╔══════════════════════════════════════════════════════╗");
                println!("║          🧬  Master Gene Generated                   ║");
                println!("╠══════════════════════════════════════════════════════╣");
                println!("║  ID:          {}", gene.id);
                println!("║  Name:        {}", gene.name);
                println!("║  Role:        {}", gene.role);
                println!("║  Status:      {}", gene.status);
                println!("║  Fingerprint: {}", gene.fingerprint);
                println!("║  Short FP:    {}", gene.short_fp);
                println!("║  Algorithm:   {}", gene.algorithm);
                println!("║  Capabilities:");
                for cap in &gene.capabilities {
                    println!("║    • {}", cap);
                }
                println!("╠══════════════════════════════════════════════════════╣");
                println!("║  ✅ Public identity: {}", dir.display());
                println!("║  🔐 Private keys:    OS keystore (service 'netgene-os')");
                println!("╚══════════════════════════════════════════════════════╝");
            }
        }

        GeneCommand::Show => {
            let genes = list_genes(&dir)?;
            if genes.is_empty() {
                println!("No genes found. Run `netgene gene init` to create one.");
                return Ok(());
            }

            if json {
                println!("{}", serde_json::to_string_pretty(&genes)?);
                return Ok(());
            }

            println!();
            println!("🧬 NetGene Identity Store — {} gene(s)", genes.len());
            println!("{}", "─".repeat(70));
            for gene in &genes {
                println!(
                    "  {} │ {} │ {} │ {} │ fp:{}",
                    gene.role, gene.name, gene.id, gene.status, gene.short_fp
                );
                if !gene.capabilities.is_empty() {
                    println!("    Caps: {}", gene.capabilities.join(", "));
                }
                if let Some(parent) = &gene.parent_id {
                    println!("    Parent: {}", parent);
                }
                println!();
            }
        }

        GeneCommand::Spawn { parent, name, role } => {
            use netgene_gene::identity::GeneRole;
            use netgene_gene::storage::load_gene;

            let gene_role = match role.as_str() {
                "agent" => GeneRole::Agent,
                "observer" => GeneRole::Observer,
                _ => GeneRole::Node,
            };

            let (parent_gene, parent_kp) = load_gene(&parent, &dir, &secrets)?;
            let caps = match gene_role {
                GeneRole::Node => vec!["node.spawn".to_string(), "network.read".to_string()],
                GeneRole::Agent => vec!["agent.run".to_string(), "network.read".to_string()],
                GeneRole::Observer => vec!["network.read".to_string()],
                _ => vec![],
            };

            let (sub, kp) = NetGene::spawn_sub_gene(&parent_gene, &parent_kp, &name, gene_role, caps)?;
            save_gene(&sub, &kp, &dir, &secrets)?;

            println!("✅ Sub-Gene spawned:");
            println!("   ID:   {}", sub.id);
            println!("   Name: {}", sub.name);
            println!("   Role: {}", sub.role);
            println!("   FP:   {}", sub.short_fp);
        }

        GeneCommand::Verify { id } => {
            use netgene_gene::storage::load_gene;
            match load_gene(&id, &dir, &secrets) {
                Ok((gene, kp)) => {
                    let probe = b"netgene-verify";
                    let sig = kp.sign(probe)?;
                    if !netgene_gene::crypto::verify_signature(&gene.public_key_bytes()?, probe, &sig) {
                        anyhow::bail!("Private key does not sign for gene {}", gene.id);
                    }
                    println!("✅ Gene verified (private key matches public key):");
                    println!("   {}", gene.display_line());

                    if let Some(parent_id) = gene.parent_id {
                        let parent = list_genes(&dir)?.into_iter().find(|g| g.id == parent_id);
                        match parent {
                            Some(p) if gene.verify_lineage(&p) => println!("   ✅ Lineage signed by parent {}", p.short_fp),
                            Some(_) => println!("   ❌ Lineage signature INVALID"),
                            None => println!("   ⚠️  Parent {} not found locally; lineage not checked", parent_id),
                        }
                    }
                }
                Err(e) => println!("❌ Verification failed: {}", e),
            }
        }

        GeneCommand::Export { id } => {
            let gene = list_genes(&dir)?
                .into_iter()
                .find(|g| g.id.to_string() == id)
                .ok_or_else(|| anyhow::anyhow!("Gene {} not found", id))?;
            println!("{}", serde_json::to_string_pretty(&gene)?);
        }

        GeneCommand::Grant { issuer, subject, caps, ttl_minutes } => {
            let (issuer_gene, issuer_kp) = load_gene(&issuer, &dir, &secrets)?;
            let subject_gene = list_genes(&dir)?
                .into_iter()
                .find(|g| g.id.to_string() == subject)
                .ok_or_else(|| anyhow::anyhow!("Subject gene {} not found", subject))?;
            if !subject_gene.verify_lineage(&issuer_gene) {
                anyhow::bail!("Only the verified parent of a gene can grant it capabilities");
            }
            let token = CapabilityToken::issue(&issuer_gene, &issuer_kp, &subject_gene, caps, chrono::Duration::minutes(ttl_minutes))?;
            let path = grants::tokens_dir().join(format!("{}.json", token.claims.token_id));
            std::fs::create_dir_all(grants::tokens_dir())?;
            std::fs::write(&path, serde_json::to_string_pretty(&token)?)?;
            println!("🎫 Token {} → {} [{}] until {}", token.claims.token_id, subject_gene.name,
                token.claims.capabilities.join(", "), token.claims.expires_at.format("%Y-%m-%d %H:%M UTC"));
            println!("   Saved to {}", path.display());
        }

        GeneCommand::Tokens => {
            let genes = list_genes(&dir)?;
            let revoked = RevocationList::load(&grants::revocation_path())?;
            let tokens = grants::list_tokens()?;
            println!("🎫 {} capability token(s)", tokens.len());
            for t in tokens {
                let status = match grants::check(&t, &genes, &revoked) {
                    Ok(()) => "VALID".to_string(),
                    Err(e) => format!("INVALID ({})", e),
                };
                println!("  • {} subject={} [{}] expires {} — {}", t.claims.token_id, t.claims.subject_gene_id,
                    t.claims.capabilities.join(", "), t.claims.expires_at.format("%Y-%m-%d %H:%M UTC"), status);
            }
        }

        GeneCommand::RevokeToken { token_id } => {
            let path = grants::revocation_path();
            let mut list = RevocationList::load(&path)?;
            if list.revoke(token_id) {
                list.save(&path)?;
                println!("⛔ Token {} revoked", token_id);
            } else {
                println!("Token {} was already revoked", token_id);
            }
        }

        GeneCommand::EnrollPresence { gene } => {
            let gene = list_genes(&dir)?
                .into_iter()
                .find(|g| g.id.to_string() == gene)
                .ok_or_else(|| anyhow::anyhow!("Gene {} not found", gene))?;
            if !netgene_presence::is_available() {
                anyhow::bail!("No hardware user verification available on this device (Windows Hello is required for now)");
            }
            println!("🔐 Follow the Windows Hello prompt to create the presence key...");
            let id = gene.id;
            let enrollment = tokio::task::spawn_blocking(move || netgene_presence::enroll(id)).await??;
            enrollment.save(&grants::presence_dir())?;
            println!("✅ Presence key enrolled for {} ({})", gene.name, enrollment.method);
            println!("   Set \"require_user_presence\": true in ~/.netgene/policy.json to require it for confirmations.");
        }

        GeneCommand::EnrollPasskey { gene } => {
            use netgene_presence::webauthn;
            let gene = list_genes(&dir)?
                .into_iter()
                .find(|g| g.id.to_string() == gene)
                .ok_or_else(|| anyhow::anyhow!("Gene {} not found", gene))?;
            let mut challenge = [0u8; 32];
            ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut challenge).map_err(|_| anyhow::anyhow!("RNG failure"))?;

            let ceremony = super::passkey::Ceremony::bind().await?;
            let origin = ceremony.origin();
            let options = webauthn::creation_options(gene.id, &gene.name, &challenge, webauthn::LOCAL_RP_ID);
            let answer = ceremony.run(super::passkey::Mode::Create, options, &format!("Create a passkey for gene {}", gene.name)).await?;
            let response: webauthn::RegistrationResponse = serde_json::from_value(answer)?;
            let credential = webauthn::verify_registration(gene.id, &response, &challenge, webauthn::LOCAL_RP_ID, &origin)?;

            let mut passkeys = webauthn::load_passkeys(&grants::presence_dir(), gene.id)?;
            passkeys.retain(|p| p.credential_id_b64 != credential.credential_id_b64);
            passkeys.push(credential.clone());
            webauthn::save_passkeys(&grants::presence_dir(), gene.id, &passkeys)?;
            println!("✅ Passkey enrolled for {} (COSE alg {}, {} passkey(s) in total)", gene.name, credential.alg, passkeys.len());
            println!("   Approve with `agent dispatch … --confirm --passkey`; set \"require_user_presence\": true to make it mandatory.");
        }

        GeneCommand::GrantOidc { issuer, subject, id_token } => {
            let config = netgene_enterprise::OidcConfig::load(&grants::netgene_home().join("enterprise.json"))?;
            let token = match id_token.strip_prefix('@') {
                Some(path) => std::fs::read_to_string(path)?,
                None => id_token,
            };
            let cache = netgene_enterprise::JwksCache::new(grants::netgene_home().join("jwks-cache.json"));
            let now = chrono::Utc::now();
            let identity = cache.verify(&config, token.trim(), now, netgene_enterprise::JwkSet::fetch).await?;
            let caps = netgene_enterprise::capabilities_for(&config, &identity);
            if caps.is_empty() {
                anyhow::bail!("ID token for {} has no role mapped to NetGene capabilities", identity.subject);
            }

            let ttl = netgene_enterprise::grant_ttl(&config, &identity, now);
            grant_from_identity(&dir, secrets.as_ref(), &issuer, &subject, &identity, caps, ttl, "oidc", &config.issuer)?;
        }

        #[cfg(feature = "saml")]
        GeneCommand::GrantSaml { issuer, subject, response } => {
            use netgene_enterprise::saml;
            let config = saml::SamlConfig::load(&grants::netgene_home().join("saml.json"))?;
            let metadata = std::fs::read_to_string(&config.idp_metadata)
                .map_err(|e| anyhow::anyhow!("Cannot read IdP metadata {}: {}", config.idp_metadata.display(), e))?;
            let response = match response.strip_prefix('@') {
                Some(path) => std::fs::read_to_string(path)?,
                None => response,
            };
            let mut replay = saml::ReplayCache::open(grants::netgene_home().join("saml-replay.json"))?;
            let now = chrono::Utc::now();
            let identity = saml::verify_saml_response(&config, &metadata, &response, &mut replay, now)?;
            let caps = saml::saml_capabilities_for(&config, &identity);
            if caps.is_empty() {
                anyhow::bail!("SAML assertion for {} has no role mapped to NetGene capabilities", identity.subject);
            }
            let ttl = saml::saml_grant_ttl(&config, &identity, now);
            grant_from_identity(&dir, secrets.as_ref(), &issuer, &subject, &identity, caps, ttl, "saml", &config.sp_entity_id)?;
        }

        GeneCommand::Import { file } => {
            let gene: NetGene = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
            gene.public_key_bytes()?;
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{}.gene.json", gene.id));
            if path.exists() {
                anyhow::bail!("Gene {} already exists locally", gene.id);
            }
            std::fs::write(&path, serde_json::to_string_pretty(&gene)?)?;
            println!("✅ Imported public identity {} ({})", gene.name, gene.short_fp);
            println!("   It joins your mesh allowlist if its lineage is signed by your Master Gene.");
        }
    }

    Ok(())
}

/// Issue a capability token to `subject` from a verified corporate identity and audit it.
#[allow(clippy::too_many_arguments)]
fn grant_from_identity(
    dir: &std::path::Path,
    secrets: &dyn netgene_gene::SecretStore,
    issuer: &str,
    subject: &str,
    identity: &netgene_enterprise::VerifiedIdentity,
    caps: Vec<String>,
    ttl: chrono::Duration,
    protocol: &str,
    source: &str,
) -> Result<()> {
    let (issuer_gene, issuer_kp) = load_gene(issuer, dir, secrets)?;
    let subject_gene = list_genes(dir)?
        .into_iter()
        .find(|g| g.id.to_string() == subject)
        .ok_or_else(|| anyhow::anyhow!("Subject gene {} not found", subject))?;
    if !subject_gene.verify_lineage(&issuer_gene) {
        anyhow::bail!("Only the verified parent of a gene can grant it capabilities");
    }
    let token = CapabilityToken::issue(&issuer_gene, &issuer_kp, &subject_gene, caps, ttl)?;
    std::fs::create_dir_all(grants::tokens_dir())?;
    std::fs::write(grants::tokens_dir().join(format!("{}.json", token.claims.token_id)), serde_json::to_string_pretty(&token)?)?;

    // Record who the capabilities came from in the audit log.
    let mut audit = netgene_policy::AuditLog::open(grants::netgene_home().join("audit.jsonl"), issuer_kp.try_clone()?)?;
    audit.append(netgene_policy::AuditRecord {
        actor_gene_id: issuer_gene.id.to_string(),
        input: format!("{} grant for {}", protocol, identity.email.clone().unwrap_or_else(|| identity.subject.clone())),
        action: Some("gene.grant".into()),
        params: serde_json::json!({
            "token_id": token.claims.token_id,
            "subject_gene": subject_gene.id,
            "protocol": protocol,
            "identity_source": source,
            "identity_subject": identity.subject,
            "roles": identity.roles,
            "capabilities": token.claims.capabilities,
        }),
        decision: "granted".into(),
        detail: format!("expires {}", token.claims.expires_at),
    })?;

    println!("🏢 {} ({}) verified by {} ({})", identity.email.as_deref().unwrap_or("-"), identity.subject, source, protocol);
    println!("🎫 Token {} → {} [{}] until {}", token.claims.token_id, subject_gene.name,
        token.claims.capabilities.join(", "), token.claims.expires_at.format("%Y-%m-%d %H:%M UTC"));
    Ok(())
}
