import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Bot, Send, X, TerminalSquare } from 'lucide-react';

interface Props {
  agent: any;
  onClose: () => void;
}

export function AgentChat({ agent, onClose }: Props) {
  const [messages, setMessages] = useState<{role: string, content: string}[]>([
    { role: 'agent', content: `[SYSTEM] Connection established with ${agent.name} (${agent.agent_type}). Awaiting input.` }
  ]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);

  const sendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || loading) return;

    const userMsg = input;
    setInput('');
    setMessages(prev => [...prev, { role: 'user', content: userMsg }]);
    setLoading(true);

    try {
      const response: string = await invoke('llm_chat', { 
        agentType: agent.agent_type,
        message: userMsg 
      });
      setMessages(prev => [...prev, { role: 'agent', content: response }]);
    } catch (err: any) {
      setMessages(prev => [...prev, { role: 'agent', content: `[ERROR] ${err}` }]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="bg-slate-900 border border-fuchsia-500/30 rounded-xl w-full max-w-2xl h-[600px] flex flex-col shadow-[0_0_30px_rgba(217,70,239,0.15)] overflow-hidden">
        
        {/* Header */}
        <div className="bg-slate-800/80 border-b border-slate-700 p-4 flex justify-between items-center">
          <div className="flex items-center gap-3">
            <Bot className="text-fuchsia-400" />
            <div>
              <h3 className="font-bold text-fuchsia-400 uppercase tracking-wider">{agent.name}</h3>
              <p className="text-xs text-slate-400 font-mono">Type: {agent.agent_type} • Status: {agent.status}</p>
            </div>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={24} />
          </button>
        </div>

        {/* Chat Log */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4 font-mono text-sm bg-[linear-gradient(to_bottom,rgba(0,0,0,0.4),rgba(0,0,0,0.8))]">
          {messages.map((m, i) => (
            <div key={i} className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}>
              <div className={`max-w-[80%] rounded-lg p-3 ${
                m.role === 'user' 
                  ? 'bg-cyan-900/40 text-cyan-100 border border-cyan-500/30 rounded-br-none' 
                  : 'bg-fuchsia-900/20 text-fuchsia-200 border border-fuchsia-500/20 rounded-bl-none'
              }`}>
                {m.role === 'agent' && <TerminalSquare size={12} className="inline mr-2 mb-1 text-fuchsia-500" />}
                {m.content}
              </div>
            </div>
          ))}
          {loading && (
            <div className="flex justify-start">
              <div className="bg-fuchsia-900/20 border border-fuchsia-500/20 rounded-lg p-3 rounded-bl-none flex gap-1">
                <div className="w-2 h-2 bg-fuchsia-500 rounded-full animate-bounce" />
                <div className="w-2 h-2 bg-fuchsia-500 rounded-full animate-bounce delay-75" />
                <div className="w-2 h-2 bg-fuchsia-500 rounded-full animate-bounce delay-150" />
              </div>
            </div>
          )}
        </div>

        {/* Input area */}
        <form onSubmit={sendMessage} className="p-4 border-t border-slate-700 bg-slate-800/50 flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Send command or query to BDI agent..."
            className="flex-1 bg-black/50 border border-slate-600 rounded px-4 py-2 text-white font-mono text-sm focus:outline-none focus:border-fuchsia-500"
            disabled={loading}
          />
          <button 
            type="submit" 
            disabled={loading || !input.trim()}
            className="bg-fuchsia-600 hover:bg-fuchsia-500 disabled:opacity-50 text-white px-4 py-2 rounded flex items-center gap-2 transition-colors"
          >
            <Send size={18} />
          </button>
        </form>
      </div>
    </div>
  );
}
