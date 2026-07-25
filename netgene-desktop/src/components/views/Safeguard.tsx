import React, { useState, useEffect } from 'react';
import { useStore } from '../../store';
import { ShieldAlert, AlertTriangle, Activity, Lock, RefreshCw, Zap } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

export function Safeguard() {
  const { safeguard, setSafeguard } = useStore();
  const [loading, setLoading] = useState(false);

  const fetchSafeguard = async () => {
    try {
      const data: any = await invoke('get_safeguard_metrics');
      setSafeguard(data);
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    fetchSafeguard();
    const timer = setInterval(fetchSafeguard, 3000);
    return () => clearInterval(timer);
  }, []);

  const handleSimulateAnomaly = async () => {
    setLoading(true);
    try {
      await invoke('dispatch_intent', { intent: 'trigger anomaly scan' });
      await fetchSafeguard();
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleSelfHeal = async () => {
    setLoading(true);
    try {
      await invoke('dispatch_intent', { intent: 'optimize network routes and heal anomaly' });
      await fetchSafeguard();
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  if (!safeguard) {
    return <div className="animate-pulse flex-1 bg-cyan-neon/10 rounded-xl" />;
  }

  return (
    <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in">
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center gap-3">
          <ShieldAlert className="text-red-500" />
          Safeguard Defense Grid
        </h2>
        <div className="flex gap-4">
          <button 
            onClick={handleSimulateAnomaly}
            disabled={loading}
            className="bg-rose-900/50 hover:bg-rose-800 text-rose-300 border border-rose-500/50 px-4 py-2 rounded-lg font-bold transition-colors text-sm flex items-center gap-2"
          >
            <AlertTriangle size={16} /> Simulate Anomaly
          </button>
          <button 
            onClick={handleSelfHeal}
            disabled={loading}
            className="bg-emerald-600 hover:bg-emerald-500 text-white px-4 py-2 rounded-lg font-bold transition-colors text-sm flex items-center gap-2 shadow-neon"
          >
            <Zap size={16} /> Trigger Self-Healing
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="border border-red-500/30 bg-black/50 p-6 rounded-xl flex items-center gap-6 shadow-[0_0_15px_rgba(239,68,68,0.2)]">
          <AlertTriangle size={48} className="text-red-500" />
          <div>
            <h3 className="text-sm font-mono opacity-60 uppercase">Anomalies Detected</h3>
            <p className="text-4xl font-bold text-red-500">{safeguard.anomalies_detected}</p>
          </div>
        </div>

        <div className="border border-emerald-500/30 bg-black/50 p-6 rounded-xl flex items-center gap-6 shadow-[0_0_15px_rgba(16,185,129,0.2)]">
          <Activity size={48} className="text-emerald-500" />
          <div>
            <h3 className="text-sm font-mono opacity-60 uppercase">Self-heals Applied</h3>
            <p className="text-4xl font-bold text-emerald-500">{safeguard.self_heals}</p>
          </div>
        </div>
      </div>

      <div className="border border-cyan-neon/30 bg-black/50 p-6 rounded-xl flex-1 shadow-neon">
         <h3 className="text-sm font-mono opacity-60 uppercase border-b border-cyan-neon/20 pb-2 mb-6">Threat Analysis & Governance Policies</h3>
         
         <div className="space-y-6">
            <div>
              <div className="flex justify-between font-mono text-sm mb-2">
                <span className="flex items-center gap-2"><Lock size={16} /> Zero Trust Policy Enforced</span>
                <span className="text-cyan-neon font-bold">{safeguard.zero_trust}</span>
              </div>
              <div className="w-full bg-black rounded-full h-2 border border-cyan-neon/30">
                <div className="bg-cyan-neon h-1.5 rounded-full" style={{ width: '100%' }}></div>
              </div>
            </div>

            <div>
              <div className="flex justify-between font-mono text-sm mb-2">
                <span>Current Threat Level</span>
                <span className={`font-bold ${safeguard.threat_level === 'HIGH' ? 'text-red-500' : 'text-emerald-400'}`}>{safeguard.threat_level}</span>
              </div>
              <div className="w-full bg-black rounded-full h-2 border border-emerald-500/30">
                <div className={`h-1.5 rounded-full ${safeguard.threat_level === 'HIGH' ? 'bg-red-500' : 'bg-emerald-500'}`} style={{ width: safeguard.threat_level === 'HIGH' ? '85%' : '20%' }}></div>
              </div>
            </div>
         </div>
      </div>
    </div>
  );
}
