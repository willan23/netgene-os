import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useStore } from '../store';
import { Send, Terminal, ShieldCheck, X } from 'lucide-react';

import type { DispatchOutcome } from '../intents';

const describe = (o: DispatchOutcome) => {
  switch (o.status) {
    case 'executed': return `[EXECUTED] ${o.action.kind} — ${o.explanation}`;
    case 'pending_confirmation': return `[CONFIRM?] ${o.action.kind} — ${o.reason}`;
    case 'denied': return `[DENIED] ${o.reason}`;
  }
};

export const IntentTerminal = () => {
  const [intent, setIntent] = useState('');
  const [loading, setLoading] = useState(false);
  const [pending, setPending] = useState<Extract<DispatchOutcome, { status: 'pending_confirmation' }> | null>(null);
  const { addLog } = useStore();

  const resolvePending = async (approve: boolean) => {
    if (!pending) return;
    setLoading(true);
    try {
      if (approve) {
        const res = await invoke<DispatchOutcome>('confirm_intent', { proposalId: pending.proposal_id });
        addLog(describe(res));
      } else {
        await invoke('reject_intent', { proposalId: pending.proposal_id });
        addLog(`[REJECTED] ${pending.action.kind}`);
      }
    } catch (error) {
      addLog(`[ERROR] ${error}`);
    } finally {
      setPending(null);
      setLoading(false);
    }
  };

  const handleDispatch = async (e?: React.FormEvent, directIntent?: string) => {
    if (e) e.preventDefault();
    const finalIntent = directIntent || intent;
    if (!finalIntent.trim()) return;
    
    setLoading(true);
    addLog(`> Dispatching intent: "${finalIntent}"`);
    try {
      const res = await invoke<DispatchOutcome>('dispatch_intent', { intent: finalIntent });
      addLog(describe(res));
      if (res.status === 'pending_confirmation') setPending(res);
    } catch (error) {
      addLog(`[ERROR] ${error}`);
    } finally {
      setLoading(false);
      if (!directIntent) setIntent('');
    }
  };

  const suggestions = [
    "Spawn 3 Edge Nodes",
    "Optimize Routes",
    "Scan for Anomalies",
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

      {pending && (
        <div className="flex items-center justify-between gap-3 border border-amber-500/50 bg-amber-950/30 rounded px-4 py-3 text-sm font-mono">
          <div className="text-amber-300">
            <span className="font-bold">{pending.action.kind}</span> {JSON.stringify(pending.action.params)}
            <div className="text-xs text-amber-200/70">{pending.reason}</div>
          </div>
          <div className="flex gap-2 shrink-0">
            <button onClick={() => resolvePending(true)} disabled={loading} className="flex items-center gap-1 bg-emerald-700 hover:bg-emerald-600 text-white px-3 py-1.5 rounded text-xs disabled:opacity-50">
              <ShieldCheck size={14} /> Confirmar
            </button>
            <button onClick={() => resolvePending(false)} disabled={loading} className="flex items-center gap-1 bg-slate-800 hover:bg-slate-700 text-slate-200 px-3 py-1.5 rounded text-xs disabled:opacity-50">
              <X size={14} /> Rejeitar
            </button>
          </div>
        </div>
      )}
      
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
