import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Landmark, Vote, Coins, Activity, Plus } from 'lucide-react';

interface DaoProposal {
    proposal_id: string;
    title: string;
    description: string;
    proposer_gene_id: string;
    proposal_type: any;
    votes_yes: number;
    votes_no: number;
    voters: string[];
    status: string;
    created_at: string;
}

export function Governance() {
    const [balance, setBalance] = useState(0);
    const [proposals, setProposals] = useState<DaoProposal[]>([]);
    const [newTitle, setNewTitle] = useState('');
    const [newDesc, setNewDesc] = useState('');
    const [showCreate, setShowCreate] = useState(false);

    useEffect(() => {
        loadData();
        const interval = setInterval(loadData, 5000);
        return () => clearInterval(interval);
    }, []);

    const loadData = async () => {
        try {
            const bal: number = await invoke('get_gene_balance');
            setBalance(bal);
            
            const props: DaoProposal[] = await invoke('get_dao_proposals');
            setProposals(props);
        } catch (e) {
            console.error(e);
        }
    };

    const handleVote = async (id: string, approve: boolean) => {
        try {
            await invoke('vote_dao_proposal', { proposalId: id, approve, weight: 10 });
            loadData();
        } catch (e) {
            console.error(e);
        }
    };

    const handleSubmit = async () => {
        if (!newTitle.trim() || !newDesc.trim()) return;
        try {
            await invoke('submit_dao_proposal', { title: newTitle, description: newDesc });
            setShowCreate(false);
            setNewTitle('');
            setNewDesc('');
            loadData();
        } catch(e) {
            console.error(e);
        }
    };

    return (
        <div className="h-full flex flex-col p-6 overflow-y-auto animate-fade-in">
            <div className="flex justify-between items-start mb-8">
                <div>
                    <h1 className="text-3xl font-bold text-shadow-neon flex items-center text-cyan-400">
                        <Landmark className="mr-3" size={32} /> DAO Governance
                    </h1>
                    <p className="text-slate-400 mt-2 text-sm">Decentralized Autonomous Organization & Proof-of-Utility</p>
                </div>
                <div className="bg-black/50 border border-cyan-500/30 p-4 rounded-xl flex items-center gap-4 shadow-neon">
                    <div className="p-3 bg-cyan-900/30 rounded-full">
                        <Coins className="text-cyan-400" />
                    </div>
                    <div>
                        <p className="text-xs font-mono text-cyan-500 uppercase">Utility Balance</p>
                        <p className="text-2xl font-bold text-white">{balance.toLocaleString()} $GENE</p>
                    </div>
                </div>
            </div>

            <div className="flex justify-between items-center mb-6">
                <h2 className="text-xl font-bold text-slate-200">Active Proposals</h2>
                <button 
                    onClick={() => setShowCreate(!showCreate)}
                    className="flex items-center gap-2 bg-cyan-600 hover:bg-cyan-500 text-white px-4 py-2 rounded font-semibold transition-colors shadow-neon"
                >
                    <Plus size={16} /> New Proposal
                </button>
            </div>

            {showCreate && (
                <div className="bg-slate-900 border border-cyan-500/50 p-6 rounded-xl mb-6 shadow-neon">
                    <h3 className="text-lg font-bold text-cyan-300 mb-4">Submit Protocol Upgrade</h3>
                    <input 
                        type="text" 
                        value={newTitle}
                        onChange={e => setNewTitle(e.target.value)}
                        placeholder="Proposal Title (e.g. Upgrade QAOA Layers)"
                        className="w-full bg-slate-950 border border-slate-700 rounded p-3 text-white font-mono text-sm focus:border-cyan-500 outline-none mb-4"
                    />
                    <textarea 
                        value={newDesc}
                        onChange={e => setNewDesc(e.target.value)}
                        placeholder="Description and technical justification..."
                        className="w-full h-32 bg-slate-950 border border-slate-700 rounded p-3 text-white font-mono text-sm focus:border-cyan-500 outline-none mb-4"
                    />
                    <button 
                        onClick={handleSubmit}
                        className="bg-cyan-700 hover:bg-cyan-600 text-white px-6 py-2 rounded font-bold transition-colors"
                    >
                        Submit to Network
                    </button>
                </div>
            )}

            <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
                {proposals.length === 0 ? (
                    <div className="col-span-full p-8 border border-slate-800 rounded-xl bg-black/20 text-center text-slate-500">
                        <Activity size={32} className="mx-auto mb-3 opacity-50" />
                        <p>No active proposals. The mesh network is quiet.</p>
                    </div>
                ) : proposals.map(p => {
                    const total = p.votes_yes + p.votes_no;
                    const yesPct = total > 0 ? (p.votes_yes / total) * 100 : 0;
                    
                    return (
                        <div key={p.proposal_id} className="bg-slate-900 border border-slate-700 rounded-xl p-6 relative overflow-hidden">
                            {p.status === 'PASSED' && <div className="absolute top-0 left-0 w-full h-1 bg-emerald-500 shadow-[0_0_10px_#10b981]"></div>}
                            {p.status === 'ACTIVE' && <div className="absolute top-0 left-0 w-full h-1 bg-cyan-500 shadow-[0_0_10px_#06b6d4]"></div>}
                            
                            <div className="flex justify-between items-start mb-2">
                                <h3 className="text-xl font-bold text-white">{p.title}</h3>
                                <span className={`text-xs px-2 py-1 rounded font-mono ${p.status === 'PASSED' ? 'bg-emerald-900/50 text-emerald-400 border border-emerald-500/50' : 'bg-cyan-900/50 text-cyan-400 border border-cyan-500/50'}`}>
                                    {p.status}
                                </span>
                            </div>
                            
                            <p className="text-sm text-slate-400 mb-6">{p.description}</p>
                            
                            <div className="mb-6">
                                <div className="flex justify-between text-xs font-mono mb-2">
                                    <span className="text-emerald-400">Approve ({p.votes_yes})</span>
                                    <span className="text-rose-400">Reject ({p.votes_no})</span>
                                </div>
                                <div className="w-full h-2 bg-slate-800 rounded-full overflow-hidden flex">
                                    <div className="h-full bg-emerald-500 transition-all duration-1000" style={{width: `${yesPct}%`}}></div>
                                    <div className="h-full bg-rose-500 transition-all duration-1000" style={{width: `${100 - yesPct}%`}}></div>
                                </div>
                                {p.status === 'ACTIVE' && <p className="text-[10px] text-slate-500 mt-2 text-center">Quorum Threshold: 100 votes</p>}
                            </div>
                            
                            {p.status === 'ACTIVE' && (
                                <div className="flex gap-4">
                                    <button 
                                        onClick={() => handleVote(p.proposal_id, true)}
                                        className="flex-1 bg-emerald-900/30 hover:bg-emerald-900/60 text-emerald-400 border border-emerald-500/30 py-2 rounded font-semibold transition-colors flex justify-center items-center gap-2"
                                    >
                                        <Vote size={16} /> Approve
                                    </button>
                                    <button 
                                        onClick={() => handleVote(p.proposal_id, false)}
                                        className="flex-1 bg-rose-900/30 hover:bg-rose-900/60 text-rose-400 border border-rose-500/30 py-2 rounded font-semibold transition-colors flex justify-center items-center gap-2"
                                    >
                                        Reject
                                    </button>
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
