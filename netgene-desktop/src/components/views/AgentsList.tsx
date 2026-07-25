import { useStore } from '../../store';
import { Bot, TerminalSquare, Activity, MessageSquare } from 'lucide-react';
import { useState, useEffect } from 'react';
import { AgentChat } from './AgentChat';
import { invoke } from '@tauri-apps/api/core';

export function AgentsList() {
  const { agents, setAgents } = useStore();
  const [selectedAgent, setSelectedAgent] = useState<any>(null);

  useEffect(() => {
    const fetchAgents = async () => {
      try {
        const list: any[] = await invoke('get_agents_state');
        setAgents(list);
      } catch (err) {
        console.error("Failed to fetch agents state", err);
      }
    };
    fetchAgents();
    const timer = setInterval(fetchAgents, 2000);
    return () => clearInterval(timer);
  }, []);

  const getAgentDesc = (type: string) => {
    switch(type) {
      case 'monitor': return 'Monitors network anomalies and logs traffic patterns.';
      case 'builder': return 'Provisions new nodes and manages topology expansion.';
      case 'optimizer': return 'Analyzes QUBO models to improve routing efficiency.';
      case 'network': return 'Ensures connectivity and peer-to-peer resilience.';
      case 'evolution': return 'Manages self-healing protocols and system updates.';
      case 'deepseek-coder-bdi': return 'Autonomous coding & vulnerability patch agent powered by DeepSeek.';
      case 'sentinel-guard-v2': return 'Zero-Trust network observer & Z-Score anomaly healer.';
      case 'swarm-coordinator': return 'Orchestrates distributed federated learning across nodes.';
      case 'crypto-trader-bot': return 'Quantum SQA & QAOA arbitrage trader.';
      default: return 'Custom BDI autonomous agent installed from NetGene Marketplace.';
    }
  };

  return (
    <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in">
      <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center gap-3">
        <Bot className="text-fuchsia-400" />
        Belief-Desire-Intention (BDI) Agents
      </h2>

      <div className="border border-cyan-neon/30 bg-black/50 rounded-xl flex-1 shadow-[0_0_20px_rgba(0,255,255,0.1)] overflow-hidden flex flex-col">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-cyan-neon/20 bg-cyan-neon/5 font-mono text-xs uppercase tracking-wider">
                <th className="p-4 opacity-70">Name</th>
                <th className="p-4 opacity-70">Type</th>
                <th className="p-4 opacity-70">Status</th>
                <th className="p-4 opacity-70">Messages</th>
                <th className="p-4 opacity-70 w-1/3">Description</th>
              </tr>
            </thead>
            <tbody className="font-mono text-sm">
              {agents.map((agent: any) => {
                const desc = getAgentDesc(agent.agent_type);
                const isIdle = agent.status === 'Idle' || agent.status === 'IDLE';
                const statusColor = isIdle ? 'text-cyan-neon' : 'text-fuchsia-400';

                return (
                  <tr 
                    key={agent.id} 
                    className="border-b border-cyan-neon/10 hover:bg-white/5 transition-colors cursor-pointer"
                    onClick={() => setSelectedAgent(agent)}
                  >
                    <td className="p-4 font-bold text-fuchsia-400 flex items-center gap-2">
                      <TerminalSquare size={16} />
                      {agent.name}
                    </td>
                    <td className="p-4 uppercase">{agent.agent_type}</td>
                    <td className={`p-4 flex items-center gap-2 ${statusColor} uppercase`}>
                      <Activity size={14} className={isIdle ? '' : 'animate-pulse'} />
                      {agent.status}
                    </td>
                    <td className="p-4">
                      <div className="flex items-center gap-2">
                        <MessageSquare size={14} className="text-cyan-400" />
                        {agent.messages_processed}
                      </div>
                    </td>
                    <td className="p-4 text-xs opacity-70">{desc}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
      
      {selectedAgent && (
        <AgentChat 
          agent={selectedAgent} 
          onClose={() => setSelectedAgent(null)} 
        />
      )}
    </div>
  );
}
