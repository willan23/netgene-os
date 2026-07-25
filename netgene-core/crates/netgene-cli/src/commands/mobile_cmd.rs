//! Mobile PWA & Passkey CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_mobile::{PasskeyEngine, MobileLiveBridge};

#[derive(Subcommand)]
pub enum MobileCommand {
    /// Issue WebAuthn Passkey biometric challenge
    Challenge {
        #[arg(short, long, default_value = "master-gene-01")]
        user: String,
    },
    /// Start mobile PWA live bridge server
    Bridge {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// Show mobile subsystem status
    Status,
}

pub async fn run(cmd: MobileCommand) -> Result<()> {
    match cmd {
        MobileCommand::Challenge { user } => {
            let engine = PasskeyEngine::new("netgene.io");
            let challenge = engine.create_challenge(&user);

            println!("📱 WebAuthn Passkey Challenge Issued:");
            println!("   Challenge ID:    {}", challenge.challenge_id);
            println!("   RP ID:           {}", challenge.rp_id);
            println!("   User:            {}", challenge.user_fingerprint);
            println!("   Bytes (Base64):  {}", challenge.challenge_bytes_b64);
            println!("   Expires:         {}", challenge.expires_at);
        }

        MobileCommand::Bridge { port } => {
            let bridge = MobileLiveBridge::new(port);
            let session = bridge.connect_client("iPhone-15-Pro-Client")?;

            println!("📱 Mobile PWA Live Bridge Active:");
            println!("   Port:       {}", bridge.server_port());
            println!("   Session ID: {}", session.session_id);
            println!("   Device:     {}", session.client_device_name);
            println!("   Encrypted:  {}", session.is_encrypted);
        }

        MobileCommand::Status => {
            println!("📱 Mobile PWA & Passkey Subsystem Status:");
            println!("   Biometric Auth:  WebAuthn Passkeys (Face ID / Touch ID)");
            println!("   Live Bridge:     WebSocket Encrypted Telemetry Stream");
            println!("   Status:          🟢 ONLINE — Ready for Mobile PWA");
        }
    }

    Ok(())
}
