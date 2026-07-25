import React, { useState, useEffect } from 'react';
import { useStore } from '../../store';
import { invoke } from '@tauri-apps/api/core';
import { Cpu, Zap, GitCommit, Activity, Download, X } from 'lucide-react';

export function QuantumModule() {
  const { quantum } = useStore();
  const [layers, setLayers] = useState(3);
  const [qasm, setQasm] = useState<string | null>(null);
  const [transpiling, setTranspiling] = useState(false);

  const [optimizing, setOptimizing] = useState(false);
  const setQuantum = useStore(state => state.setQuantum);

  const handleRunOptimization = async () => {
    setOptimizing(true);
    try {
      await invoke('optimize_routes', { nodes: 6, layers });
      const updated = await invoke('get_quantum_status');
      setQuantum(updated);
    } catch (e) {
      console.error(e);
    } finally {
      setOptimizing(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in relative">
      {qasm && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-6">
            <div className="bg-slate-900 border border-purple-500/50 rounded-xl p-6 w-full max-w-3xl shadow-[0_0_30px_rgba(168,85,247,0.2)]">
                <div className="flex justify-between items-center mb-4">
                    <h2 className="text-xl font-bold text-purple-400 flex items-center gap-2">
                        <Cpu /> Transpiled OpenQASM 3.0
                    </h2>
                    <button onClick={() => setQasm(null)} className="text-slate-400 hover:text-white transition-colors">
                        <X />
                    </button>
                </div>
                <p className="text-slate-400 text-sm mb-4">
                    Ready to be dispatched to physical QPUs (IBM Quantum, AWS Braket, IonQ).
                </p>
                <pre className="bg-black/50 p-4 rounded-lg font-mono text-sm text-emerald-400 border border-slate-800 overflow-x-auto">
                    {qasm}
                </pre>
            </div>
        </div>
      )}

      <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center gap-3 text-fuchsia-400">
        <Cpu className="text-fuchsia-400" />
        Quantum Mesh Optimizer
      </h2>

      <div className="grid grid-cols-3 gap-6">
        <div className="border border-fuchsia-500/30 bg-black/50 p-6 rounded-xl flex items-center gap-4 shadow-[0_0_15px_rgba(217,70,239,0.2)]">
          <GitCommit size={32} className="text-fuchsia-500" />
          <div>
            <h3 className="text-xs font-mono opacity-60 uppercase">Algorithm</h3>
            <p className="text-lg font-bold">{quantum.algorithm}</p>
          </div>
        </div>

        <div className="border border-cyan-neon/30 bg-black/50 p-6 rounded-xl flex items-center gap-4 shadow-neon">
          <Zap size={32} className="text-cyan-neon" />
          <div>
            <h3 className="text-xs font-mono opacity-60 uppercase">Improvement</h3>
            <p className="text-2xl font-bold text-cyan-neon">+{quantum.improvement}%</p>
            <button 
                onClick={handleExportQasm}
                disabled={transpiling}
                className="mt-2 w-full flex justify-center items-center gap-2 bg-purple-900/40 hover:bg-purple-800/60 border border-purple-500/50 text-purple-300 py-1 px-3 rounded-lg text-xs font-bold transition-colors disabled:opacity-50"
            >
                {transpiling ? <Activity className="animate-spin" size={14} /> : <Download size={14} />}
                Export OpenQASM
            </button>
          </div>
        </div>
        
        <div className="border border-emerald-500/30 bg-black/50 p-6 rounded-xl flex items-center gap-4 shadow-[0_0_15px_rgba(16,185,129,0.2)]">
          <div className="h-4 w-4 bg-emerald-500 rounded-full animate-pulse" />
          <div>
            <h3 className="text-xs font-mono opacity-60 uppercase">Status</h3>
            <p className="text-lg font-bold text-emerald-500">{quantum.status}</p>
          </div>
        </div>
      </div>

      <div className="border border-fuchsia-500/30 bg-black/50 p-6 rounded-xl flex-1 shadow-[0_0_15px_rgba(217,70,239,0.1)] flex flex-col justify-between">
         <div>
            <h3 className="text-sm font-mono opacity-60 uppercase border-b border-fuchsia-500/20 pb-2 mb-4">QAOA Interactive Solver Controls</h3>
            <div className="grid grid-cols-2 gap-6 items-center mb-6">
                <div>
                    <label className="block text-xs font-mono text-slate-400 mb-2">Circuit Layers (p = {layers})</label>
                    <input 
                        type="range" 
                        min="1" 
                        max="10" 
                        value={layers} 
                        onChange={e => setLayers(Number(e.target.value))}
                        className="w-full accent-fuchsia-500 cursor-pointer"
                    />
                </div>
                <div className="flex justify-end">
                    <button 
                        onClick={handleRunOptimization}
                        disabled={optimizing}
                        className="bg-fuchsia-600 hover:bg-fuchsia-500 disabled:opacity-50 text-white px-6 py-3 rounded-lg font-bold transition-colors shadow-[0_0_15px_rgba(217,70,239,0.4)] flex items-center gap-2"
                    >
                        {optimizing ? <Activity className="animate-spin" size={18} /> : <Zap size={18} />}
                        {optimizing ? 'Solving QUBO Matrix...' : 'Run QAOA Optimization'}
                    </button>
                </div>
            </div>
         </div>

         <div className="font-mono text-sm bg-black/80 p-4 border border-fuchsia-500/20 rounded">
            <p className="text-fuchsia-400 mb-2">Engine: {quantum.solver}</p>
            <p className="opacity-70">The Quantum Approximate Optimization Algorithm (QAOA) dynamically constructs an nalgebra QUBO matrix based on active network topology, solving graph partition cost functions natively in Rust.</p>
         </div>
      </div>
    </div>
  );
}
