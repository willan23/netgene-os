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
    // Same home resolution as the gene and store crates.
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let vault_dir = home.join(".netgene").join("vault");
    
    let vault = NetGeneVault::open(vault_dir, &netgene_gene::OsKeyring::new())?;

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
