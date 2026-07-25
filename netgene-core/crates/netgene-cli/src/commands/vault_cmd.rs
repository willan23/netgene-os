use clap::Subcommand;
use anyhow::Result;
use netgene_vault::NetGeneVault;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum VaultCommand {
    /// Store a file in the vault
    Store {
        #[arg(short, long)]
        filename: String,
        #[arg(short, long)]
        data: String,
    },
    /// List files in the vault
    List,
}

pub async fn run(command: VaultCommand) -> Result<()> {
    // Determine standard vault path
    let vault_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".netgene")
        .join("vault");
    
    let vault = NetGeneVault::new(vault_dir)?;

    match command {
        VaultCommand::Store { filename, data } => {
            let meta = vault.store_file(filename, data.as_bytes())?;
            println!("📦 Stored file in vault with ID: {}", meta.file_id);
        }
        VaultCommand::List => {
            let files = vault.list_files()?;
            println!("📦 Vault Files:");
            for f in files {
                println!("- {} (ID: {}, Size: {} bytes)", f.filename, f.file_id, f.size_bytes);
            }
        }
    }
    Ok(())
}
