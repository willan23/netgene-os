import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Download, Cpu, Shield, Globe, Star, Plus, X, Check } from 'lucide-react';
import { useStore } from '../../store';

interface AgentManifest {
    id: string;
    name: string;
    version: string;
    description: string;
    author: string;
    capabilities: string[];
    publisher_gene_id: string;
}

interface Reputation {
    score: number | null;
    trusted_reviews: number;
    untrusted_reviews: number;
    rejected_reviews: number;
}

interface Gene {
    id: string;
    name: string;
    short_fp: string;
}

export function MarketplaceView() {
    const [agents, setAgents] = useState<AgentManifest[]>([]);
    const [installing, setInstalling] = useState<string | null>(null);
    const [installedIds, setInstalledIds] = useState<string[]>([]);
    const [showPublish, setShowPublish] = useState(false);
    const [genes, setGenes] = useState<Gene[]>([]);
    const [reputations, setReputations] = useState<Record<string, Reputation>>({});
    const [formData, setFormData] = useState({
        geneId: '', id: '', name: '', version: '1.0.0', description: '', capabilities: ''
    });

    const setStoreAgents = useStore(state => state.setAgents);

    useEffect(() => {
        loadAgents();
    }, []);

    const loadAgents = async () => {
        try {
            const result = await invoke<AgentManifest[]>('list_marketplace_agents');
            setAgents(result);

            const localGenes = await invoke<Gene[]>('list_genes');
            setGenes(localGenes);

            setReputations(await invoke<Record<string, Reputation>>('list_agent_reputations'));

            const activeAgents: any[] = await invoke('get_agents_state');
            const activeTypes = activeAgents.map(a => a.agent_type);
            setInstalledIds(activeTypes);
        } catch (error) {
            console.error("Failed to load marketplace agents", error);
        }
    };

    const handleInstall = async (id: string) => {
        setInstalling(id);
        try {
            await invoke('install_agent', { agentId: id });
            setInstalledIds(prev => [...prev, id]);
            const updatedActive: any[] = await invoke('get_agents_state');
            setStoreAgents(updatedActive);
        } catch (error) {
            console.error("Failed to install agent", error);
            alert("Failed to install agent: " + error);
        } finally {
            setInstalling(null);
        }
    };

    const handlePublish = async () => {
        if (!formData.geneId || !formData.id || !formData.name) return;
        try {
            await invoke('publish_agent', {
                geneId: formData.geneId,
                id: formData.id,
                name: formData.name,
                version: formData.version,
                description: formData.description,
                capabilities: formData.capabilities.split(',').map(s => s.trim()).filter(Boolean)
            });
            setShowPublish(false);
            setFormData({ geneId: '', id: '', name: '', version: '1.0.0', description: '', capabilities: '' });
            loadAgents();
        } catch (error) {
            console.error("Failed to publish agent", error);
            alert("Failed to publish agent: " + error);
        }
    };

    return (
        <div className="h-full flex flex-col p-6 overflow-y-auto">
            <div className="mb-6 flex justify-between items-start">
                <div>
                    <h1 className="text-2xl font-bold text-white mb-2 flex items-center">
                        <Globe className="mr-3 text-cyan-400" /> Agent Marketplace
                    </h1>
                    <p className="text-slate-400">Expand your NetSphere Kernel with specialized autonomous agents.</p>
                </div>
                <button 
                    onClick={() => setShowPublish(true)}
                    className="bg-cyan-900/50 hover:bg-cyan-800 text-cyan-300 border border-cyan-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center gap-2 shadow-neon"
                >
                    <Plus size={16} /> Publish Agent
                </button>
            </div>

            {showPublish && (
                <div className="mb-8 p-6 bg-slate-900/80 border border-cyan-500/50 rounded-xl relative animate-fade-in shadow-neon">
                    <button 
                        onClick={() => setShowPublish(false)}
                        className="absolute top-4 right-4 text-slate-400 hover:text-white"
                    >
                        <X size={20} />
                    </button>
                    <h2 className="text-lg font-bold text-cyan-300 mb-2">Publish New Agent</h2>
                    <p className="text-sm text-slate-400 mb-4">O manifesto é assinado com a chave da Gene escolhida (guardada no cofre do sistema operativo).</p>
                    
                    <div className="grid grid-cols-2 gap-4 mb-4 text-sm text-slate-300">
                        <div className="col-span-2">
                            <label className="block mb-1 text-slate-400">Gene que assina</label>
                            <select className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" value={formData.geneId} onChange={e => setFormData({...formData, geneId: e.target.value})}>
                                <option value="">{genes.length === 0 ? 'Nenhuma Gene local — crie com `netgene gene init`' : 'Escolha uma Gene'}</option>
                                {genes.map(g => <option key={g.id} value={g.id}>{g.name} ({g.short_fp})</option>)}
                            </select>
                        </div>
                        <div>
                            <label className="block mb-1 text-slate-400">Agent ID (a-z, 0-9, -)</label>
                            <input type="text" className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" placeholder="ex: log-summarizer" value={formData.id} onChange={e => setFormData({...formData, id: e.target.value})} />
                        </div>
                        <div>
                            <label className="block mb-1 text-slate-400">Nome</label>
                            <input type="text" className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" placeholder="ex: Log Summarizer" value={formData.name} onChange={e => setFormData({...formData, name: e.target.value})} />
                        </div>
                        <div>
                            <label className="block mb-1 text-slate-400">Versão</label>
                            <input type="text" className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" placeholder="1.0.0" value={formData.version} onChange={e => setFormData({...formData, version: e.target.value})} />
                        </div>
                        <div className="col-span-2">
                            <label className="block mb-1 text-slate-400">Descrição</label>
                            <input type="text" className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" placeholder="Descreva as capacidades operacionais do Agente..." value={formData.description} onChange={e => setFormData({...formData, description: e.target.value})} />
                        </div>
                        <div className="col-span-2">
                            <label className="block mb-1 text-slate-400">Capacidades (separadas por vírgula)</label>
                            <input type="text" className="w-full bg-slate-950 border border-slate-700 rounded p-2 focus:border-cyan-500 outline-none" placeholder="ex: logs.read, llm.chat" value={formData.capabilities} onChange={e => setFormData({...formData, capabilities: e.target.value})} />
                        </div>
                    </div>
                    
                    <button 
                        onClick={handlePublish}
                        className="bg-cyan-600 hover:bg-cyan-500 text-white px-6 py-2 rounded font-semibold transition-colors"
                    >
                        Submit to Store
                    </button>
                </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {agents.map(agent => (
                    <div key={agent.id} className="bg-slate-900 border border-slate-700/50 rounded-lg p-5 flex flex-col relative overflow-hidden group hover:border-cyan-500/50 transition-colors">
                        <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-cyan-500 to-transparent opacity-0 group-hover:opacity-100 transition-opacity"></div>
                        
                        <div className="flex justify-between items-start mb-4">
                            <div className="p-3 bg-slate-800 rounded-lg">
                                {agent.id.includes('guard') ? <Shield className="text-emerald-400" size={24} /> : <Cpu className="text-purple-400" size={24} />}
                            </div>
                            <div className="flex flex-col items-end gap-1">
                                <span className="text-xs font-mono text-slate-500 bg-slate-950 px-2 py-1 rounded">v{agent.version}</span>
                                <span className="text-xs font-mono text-yellow-400" title={`${reputations[agent.id]?.untrusted_reviews ?? 0} avaliações de fora da sua linhagem não contam`}>
                                    {reputations[agent.id]?.score != null
                                        ? `★ ${reputations[agent.id]!.score!.toFixed(1)} (${reputations[agent.id]!.trusted_reviews})`
                                        : '★ sem avaliações confiáveis'}
                                </span>
                            </div>
                        </div>
                        
                        <h3 className="text-lg font-bold text-white mb-1">{agent.name}</h3>
                        <p className="text-sm text-slate-400 mb-4 flex-grow">{agent.description}</p>
                        
                        <div className="flex flex-wrap gap-2 mb-6">
                            {agent.capabilities.map(cap => (
                                <span key={cap} className="text-xs px-2 py-1 bg-slate-800 text-cyan-300 rounded border border-cyan-900/30">
                                    {cap}
                                </span>
                            ))}
                        </div>
                        
                        <div className="flex items-center justify-between mt-auto">
                            <span className="text-xs text-slate-500 flex items-center">
                                <Star size={12} className="mr-1 text-yellow-500" /> {agent.author}
                            </span>
                            {installedIds.includes(agent.id) ? (
                                <button 
                                    disabled
                                    className="flex items-center bg-emerald-950 border border-emerald-500/50 text-emerald-300 px-4 py-2 rounded text-sm font-semibold cursor-default"
                                >
                                    <Check size={16} className="mr-2 text-emerald-400" /> Installed
                                </button>
                            ) : (
                                <button 
                                    onClick={() => handleInstall(agent.id)}
                                    disabled={installing === agent.id}
                                    className="flex items-center bg-cyan-600 hover:bg-cyan-500 text-white px-4 py-2 rounded transition-colors text-sm font-semibold disabled:opacity-50"
                                >
                                    {installing === agent.id ? 'Installing...' : <><Download size={16} className="mr-2" /> Install</>}
                                </button>
                            )}
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
