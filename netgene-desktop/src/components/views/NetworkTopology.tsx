import React, { useEffect, useState } from 'react';
import { useStore } from '../../store';
import { Network, Plus, RefreshCw, Cpu } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

export function NetworkTopology() {
  const { topology, setTopology } = useStore();
  const [loading, setLoading] = useState(false);

  const fetchTopology = async () => {
    try {
      const data: any[] = await invoke('get_network_topology');
      setTopology(data);
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    fetchTopology();
    const timer = setInterval(fetchTopology, 3000);
    return () => clearInterval(timer);
  }, []);

  const handleSpawnNode = async () => {
    setLoading(true);
    try {
      await invoke('dispatch_intent', { intent: 'provision 1 mesh node' });
      await fetchTopology();
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in">
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center gap-3">
          <Network className="text-cyan-neon" />
          Network Topology Grid
        </h2>
        <div className="flex gap-4">
          <button 
            onClick={fetchTopology}
            className="p-2 bg-slate-900 border border-slate-700 hover:border-cyan-500 rounded text-slate-300 transition-colors"
            title="Refresh Network"
          >
            <RefreshCw size={18} />
          </button>
          <button 
            onClick={handleSpawnNode}
            disabled={loading}
            className="bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-white px-4 py-2 rounded-lg font-bold transition-colors shadow-neon flex items-center gap-2 text-sm"
          >
            {loading ? <RefreshCw className="animate-spin" size={16} /> : <Plus size={16} />}
            Provision Mesh Node
          </button>
        </div>
      </div>

      <div className="border border-cyan-neon/30 bg-black/50 rounded-xl flex-1 shadow-neon overflow-hidden flex flex-col">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-cyan-neon/20 bg-cyan-neon/5 font-mono text-xs uppercase tracking-wider">
                <th className="p-4 opacity-70">Node Name / ID</th>
                <th className="p-4 opacity-70">Address / IP</th>
                <th className="p-4 opacity-70">Layer</th>
                <th className="p-4 opacity-70">Capabilities</th>
                <th className="p-4 opacity-70">Status</th>
                <th className="p-4 opacity-70">Ed25519 Fingerprint</th>
              </tr>
            </thead>
            <tbody className="font-mono text-sm">
              {(!topology || topology.length === 0) ? (
                <tr>
                  <td colSpan={6} className="p-8 text-center text-slate-500">
                    No active nodes found. Click "Provision Mesh Node" to deploy nodes to Sled DB.
                  </td>
                </tr>
              ) : (
                topology.map((node) => (
                  <tr key={node.id} className="border-b border-cyan-neon/10 hover:bg-white/5 transition-colors">
                    <td className="p-4 font-bold text-white flex items-center gap-2">
                      <Cpu size={16} className="text-cyan-400" />
                      {node.name || node.id.substring(0, 12)}
                    </td>
                    <td className="p-4 text-cyan-300 text-xs">{node.address || '127.0.0.1:8000'}</td>
                    <td className="p-4 text-fuchsia-400 font-bold">{node.layer || 'Mesh'}</td>
                    <td className="p-4">
                      <span className="bg-slate-800 text-slate-300 px-2 py-1 rounded text-xs">
                        {node.quantum_enabled ? 'QAOA + ed25519' : 'Standard Node'}
                      </span>
                    </td>
                    <td className="p-4">
                      <span className={`px-2 py-1 rounded text-xs font-bold uppercase ${node.status === 'Degraded' ? 'bg-amber-900/40 text-amber-400 border border-amber-500/30' : 'bg-emerald-900/40 text-emerald-400 border border-emerald-500/30'}`}>
                        {node.status || 'ONLINE'}
                      </span>
                    </td>
                    <td className="p-4 text-xs opacity-60 text-cyan-500 truncate max-w-[150px]">
                      {node.gene_fingerprint || node.id}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
