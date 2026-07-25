import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Network, Orbit, AlertTriangle, Send, BrainCircuit, Activity, Cloud, Wifi, Users } from 'lucide-react';

interface FederatedModel {
    threats: string[];
    sync_count: number;
    global_knowledge_size: number;
}

export function SwarmIntelligence() {
    const [model, setModel] = useState<FederatedModel | null>(null);
    const [newThreat, setNewThreat] = useState('');
    const [syncing, setSyncing] = useState(false);
    const [injecting, setInjecting] = useState(false);
    
    // Cloud State
    const [cloudEnabled, setCloudEnabled] = useState(false);
    const [peers, setPeers] = useState(0);
    const [peerAddress, setPeerAddress] = useState('');

    useEffect(() => {
        loadModel();
        const interval = setInterval(loadModel, 3000);
        return () => clearInterval(interval);
    }, []);

    const loadModel = async () => {
        try {
            const data: FederatedModel = await invoke('get_federated_model');
            setModel(data);
            
            if (cloudEnabled) {
                const count: number = await invoke('get_connected_peers');
                setPeers(count);
            }
        } catch (e) {
            console.error(e);
        }
    };

    const handleInjectThreat = async () => {
        if (!newThreat.trim()) return;
        setInjecting(true);
        try {
            await invoke('inject_local_threat', { threat: newThreat });
            setNewThreat('');
            await loadModel();
        } catch(e) {
            console.error(e);
        }
        setInjecting(false);
    };

    const handleSwarmSync = async () => {
        setSyncing(true);
        try {
            await invoke('trigger_swarm_sync');
            await loadModel();
        } catch(e) {
            console.error(e);
        }
        setSyncing(false);
    };

    const handleEnableCloud = async () => {
        try {
            await invoke('enable_cloud_mesh', { port: 8000 });
            setCloudEnabled(true);
        } catch(e) {
            console.error(e);
        }
    };

    const handleConnectPeer = async () => {
        if (!peerAddress.trim()) return;
        try {
            await invoke('connect_to_peer', { address: peerAddress });
            setPeerAddress('');
            loadModel();
        } catch(e) {
            console.error(e);
        }
    };

    return (
        <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in overflow-y-auto p-2">
            <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center justify-between w-full">
                <span className="flex items-center gap-3">
                    <Network className="text-cyan-neon" />
                    Agent Swarm Intelligence (Federated Learning)
                </span>
                
                {/* Cloud Mesh Toggle */}
                <div className="flex items-center gap-4 text-sm font-mono bg-black/40 px-4 py-2 rounded-xl border border-cyan-neon/30">
                    <span className="text-cyan-300">Cloud Mesh P2P</span>
                    <button 
                        onClick={cloudEnabled ? undefined : handleEnableCloud}
                        className={`w-12 h-6 rounded-full p-1 transition-colors ${cloudEnabled ? 'bg-cyan-500' : 'bg-slate-700'}`}
                    >
                        <div className={`w-4 h-4 rounded-full bg-white transition-transform ${cloudEnabled ? 'translate-x-6' : 'translate-x-0'}`} />
                    </button>
                    {cloudEnabled && (
                        <div className="flex items-center gap-2 text-fuchsia-400 ml-4 border-l border-cyan-900 pl-4">
                            <Users size={16} /> Peers: {peers}
                        </div>
                    )}
                </div>
            </h2>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                
                {/* Global Model State */}
                <div className="border border-cyan-neon/30 bg-black/50 p-6 rounded-xl flex flex-col gap-6 shadow-neon relative overflow-hidden">
                    <div className="absolute -right-10 -top-10 opacity-10">
                        <Orbit size={200} className={`text-cyan-400 ${syncing ? 'animate-spin' : ''}`} style={{ animationDuration: '4s' }} />
                    </div>
                    
                    <h3 className="text-sm font-mono opacity-60 uppercase border-b border-cyan-neon/20 pb-2 flex items-center gap-2">
                        <BrainCircuit size={16}/> Global Federated Model
                    </h3>
                    
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-xs font-mono opacity-50 mb-1">Total Sync Cycles</p>
                            <p className="font-bold text-3xl text-cyan-400">{model?.sync_count || 0}</p>
                        </div>
                        <div className="text-right">
                            <p className="text-xs font-mono opacity-50 mb-1">Knowledge Size</p>
                            <p className="font-bold text-3xl text-fuchsia-400">{model?.global_knowledge_size || 0} Entities</p>
                        </div>
                    </div>

                    <div className="flex-1 min-h-[150px]">
                        <p className="text-xs font-mono opacity-50 mb-2">Known Global Threats (Propagated to all Agents):</p>
                        {model?.threats.length === 0 ? (
                            <div className="flex items-center justify-center h-full border border-dashed border-cyan-900/50 rounded-lg text-slate-500 text-sm">
                                No threats registered in the global model yet.
                            </div>
                        ) : (
                            <ul className="space-y-2 max-h-48 overflow-y-auto pr-2 custom-scrollbar">
                                {model?.threats.map((t, i) => (
                                    <li key={i} className="bg-red-950/30 border border-red-900/50 p-2 rounded text-xs font-mono text-red-400 flex items-center gap-2">
                                        <AlertTriangle size={14} /> {t}
                                    </li>
                                ))}
                            </ul>
                        )}
                    </div>

                    <button 
                        onClick={handleSwarmSync} 
                        disabled={syncing}
                        className="mt-4 bg-cyan-900/50 hover:bg-cyan-800 text-cyan-300 border border-cyan-500/50 px-4 py-3 rounded text-sm w-full transition-colors flex justify-center items-center gap-2 font-bold uppercase tracking-wider disabled:opacity-50"
                    >
                        {syncing ? <Activity className="animate-pulse" /> : <Network />}
                        {syncing ? 'Synchronizing Hive Mind...' : 'Trigger Swarm Sync'}
                    </button>
                </div>

                {/* Local Node Simulator */}
                <div className="border border-fuchsia-500/30 bg-black/50 p-6 rounded-xl flex flex-col gap-6 shadow-[0_0_15px_rgba(217,70,239,0.1)]">
                    <h3 className="text-sm font-mono opacity-60 uppercase border-b border-fuchsia-500/20 pb-2 text-fuchsia-400 flex items-center gap-2">
                        <AlertTriangle size={16}/> Local Threat Injection
                    </h3>
                    
                    <p className="text-xs text-slate-400 leading-relaxed">
                        Simulate a localized attack on a specific node. By injecting a threat here, it stays "local" until the next <b>Swarm Sync</b>, where the agent shares this knowledge with the Kernel to update all other BDI Agents across the NetGene network.
                    </p>

                    <div className="mt-auto">
                        <label className="text-xs font-mono text-fuchsia-400 mb-2 block">Detected Threat Signature / IP</label>
                        <div className="flex gap-2">
                            <input 
                                type="text" 
                                value={newThreat}
                                onChange={e => setNewThreat(e.target.value)}
                                placeholder="e.g. 192.168.1.100 - DDoS Pattern"
                                className="flex-1 bg-slate-900 border border-slate-700 rounded p-2 text-white font-mono text-sm focus:border-fuchsia-500 outline-none"
                            />
                            <button 
                                onClick={handleInjectThreat}
                                disabled={!newThreat.trim() || injecting}
                                className="bg-fuchsia-900/50 hover:bg-fuchsia-800 text-fuchsia-300 border border-fuchsia-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center gap-2 disabled:opacity-50"
                            >
                                <Send size={16} /> Inject
                            </button>
                        </div>
                    </div>
                </div>

            </div>

            {/* Cloud P2P Connect */}
            {cloudEnabled && (
                <div className="border border-cyan-500/30 bg-black/50 p-6 rounded-xl flex items-center gap-4 shadow-neon animate-fade-in">
                    <Cloud className="text-cyan-400" size={24} />
                    <div>
                        <h3 className="text-sm font-mono text-cyan-300 uppercase">Connect to Remote Mesh Peer</h3>
                        <p className="text-xs text-slate-400">Join a remote NetGene Cloud cluster by IP (e.g. 192.168.1.50:8000)</p>
                    </div>
                    <div className="flex-1 flex gap-2 ml-4">
                        <input 
                            type="text" 
                            value={peerAddress}
                            onChange={e => setPeerAddress(e.target.value)}
                            placeholder="IP:PORT"
                            className="bg-slate-900 border border-slate-700 rounded p-2 text-white font-mono text-sm focus:border-cyan-500 outline-none w-64"
                        />
                        <button 
                            onClick={handleConnectPeer}
                            disabled={!peerAddress.trim()}
                            className="bg-cyan-900/50 hover:bg-cyan-800 text-cyan-300 border border-cyan-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center gap-2 disabled:opacity-50"
                        >
                            <Wifi size={16} /> Connect
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}
