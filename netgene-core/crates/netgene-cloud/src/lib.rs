use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::info;
use anyhow::Result;

pub struct MeshNode {
    pub port: u16,
    pub peers: Arc<Mutex<HashMap<String, TcpStream>>>,
}

impl MeshNode {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Inicia o nó Cloud P2P, aguardando por conexões
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("☁️ NetGene Cloud (P2P Mesh) escutando em {}", addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let peer_ip = peer_addr.to_string();
            info!("🔗 Peer conectado: {}", peer_ip);
            
            let mut peers = self.peers.lock().await;
            peers.insert(peer_ip.clone(), stream);
            
            // Num sistema real, iniciaríamos um tokio::spawn para ler desta stream
        }
    }

    /// Liga-se a um Peer manualmente
    pub async fn connect_to_peer(&self, address: &str) -> Result<()> {
        info!("Tentando conectar ao peer {}", address);
        let stream = TcpStream::connect(address).await?;
        let mut peers = self.peers.lock().await;
        peers.insert(address.to_string(), stream);
        info!("✅ Conectado com sucesso ao peer P2P {}", address);
        Ok(())
    }

    pub async fn get_connected_peers_count(&self) -> usize {
        self.peers.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mesh_node_creation() {
        let node = MeshNode::new(9001);
        assert_eq!(node.port, 9001);
        assert_eq!(node.get_connected_peers_count().await, 0);
    }

    #[tokio::test]
    async fn test_mesh_node_peer_count_initial() {
        let node = MeshNode::new(9002);
        assert_eq!(node.get_connected_peers_count().await, 0);
    }
}
