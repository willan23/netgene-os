import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useStore } from '../store';
import { Send, Terminal } from 'lucide-react';

export const IntentTerminal = () => {
  const [intent, setIntent] = useState('');
  const [loading, setLoading] = useState(false);
  const { addLog } = useStore();

  const handleDispatch = async (e?: React.FormEvent, directIntent?: string) => {
    if (e) e.preventDefault();
    const finalIntent = directIntent || intent;
    if (!finalIntent.trim()) return;
    
    setLoading(true);
    addLog(`> Dispatching intent: "${finalIntent}"`);
    try {
      const res = await invoke<string>('dispatch_intent', { intent: finalIntent });
      addLog(`[SUCCESS] ${res}`);
    } catch (error) {
      addLog(`[ERROR] ${error}`);
    } finally {
      setLoading(false);
      if (!directIntent) setIntent('');
    }
  };

  const suggestions = [
    "Spawn 3 Quantum Nodes",
    "Optimize Routes",
    "Heal Network",
    "System Status"
  ];

  return (
    <div className="bg-surface/50 p-4 border border-cyan-neon/30 rounded-xl flex flex-col gap-4">
      <form onSubmit={(e) => handleDispatch(e)} className="flex gap-2">
        <div className="flex-1 relative">
          <div className="absolute inset-y-0 left-3 flex items-center pointer-events-none">
             <Terminal size={16} className="text-cyan-neon/50" />
          </div>
          <input
            type="text"
            value={intent}
            onChange={(e) => setIntent(e.target.value)}
            placeholder="Type your intent... (e.g., spawn nodes, optimize)"
            className="w-full bg-background border border-cyan-neon/30 rounded px-10 py-3 text-cyan-neon focus:outline-none focus:border-cyan-neon focus:shadow-[0_0_10px_rgba(0,255,255,0.2)] font-mono text-sm transition-all"
            disabled={loading}
          />
        </div>
        <button 
          type="submit" 
          disabled={loading}
          className="bg-cyan-neon/20 border border-cyan-neon text-cyan-neon px-6 py-2 rounded flex items-center justify-center hover:bg-cyan-neon hover:text-background transition-colors disabled:opacity-50"
        >
          <Send size={18} />
        </button>
      </form>
      
      <div className="flex gap-2 overflow-x-auto pb-1 scrollbar-hide">
        {suggestions.map((cmd) => (
          <button
            key={cmd}
            onClick={() => handleDispatch(undefined, cmd)}
            disabled={loading}
            className="whitespace-nowrap bg-black/60 border border-cyan-neon/20 text-cyan-neon/70 px-3 py-1.5 rounded-full text-xs font-mono hover:bg-cyan-neon/10 hover:border-cyan-neon/50 hover:text-cyan-neon transition-colors"
          >
            {cmd}
          </button>
        ))}
      </div>
    </div>
  );
};
