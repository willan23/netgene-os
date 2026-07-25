use clap::Subcommand;
use anyhow::Result;
use netgene_tpm::TpmEnclave;

#[derive(Subcommand)]
pub enum TpmCommand {
    /// Show TPM Hardware status
    Status,
    /// Seal a payload to TPM
    Seal {
        #[arg(short, long)]
        payload: String,
    },
}

pub async fn run(command: TpmCommand) -> Result<()> {
    let tpm = TpmEnclave::new();
    match command {
        TpmCommand::Status => {
            println!("🔒 TPM Hardware Backed: {}", tpm.is_hardware_backed());
        }
        TpmCommand::Seal { payload } => {
            let pcr_state = b"current_pcr";
            let sealed = tpm.seal_master_gene(payload.as_bytes(), pcr_state)?;
            println!("🔒 Payload sealed successfully. Size: {} bytes", sealed.len());
        }
    }
    Ok(())
}
