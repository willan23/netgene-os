import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useStore } from './store';
import { Sidebar } from './components/Sidebar';
import { DashboardView } from './components/views/DashboardView';
import { GeneLayer } from './components/views/GeneLayer';
import { Safeguard } from './components/views/Safeguard';
import { QuantumModule } from './components/views/QuantumModule';
import { NetworkTopology } from './components/views/NetworkTopology';
import { AgentsList } from './components/views/AgentsList';
import { MarketplaceView } from './components/views/MarketplaceView';
import { SwarmIntelligence } from './components/views/SwarmIntelligence';
import { Governance } from './components/views/Governance';
import { NeuralLink } from './components/views/NeuralLink';
import { Vault } from './components/views/Vault';
import { LockScreen } from './components/LockScreen';

function App() {
  const [isUnlocked, setIsUnlocked] = useState(false);
  const { activeTab, setMetrics, addLog, setLogs, setAgents, setQuantum, setSafeguard, setTopology } = useStore();

  useEffect(() => {
    const loadInitialData = async () => {
      try {
        const events: any[] = await invoke('get_stored_events', { limit: 100 });
        if (events.length > 0) {
          const historicalLogs = events.map(e => `[${new Date(e.timestamp).toLocaleTimeString()}] ${e.details}`);
          setLogs(historicalLogs);
          addLog('[SYSTEM] Loaded historical events.');
        }

        const topology: any[] = await invoke('get_network_topology');
        setTopology(topology);

        const agents: any[] = await invoke('get_agents_state');
        setAgents(agents);

        const quantum: any = await invoke('get_quantum_status');
        setQuantum(quantum);

        const safeguard: any = await invoke('get_safeguard_metrics');
        setSafeguard(safeguard);

      } catch (err) {
        console.error("Initialization failed:", err);
        addLog(`[ERROR] Init failed: ${err}`);
      }
    };
    loadInitialData();

    const unlistenTick = listen('network-tick', (event: any) => {
      setMetrics(event.payload);
      if (event.payload.health) {
         setMetrics({ health: event.payload.health });
      }
    });

    const unlistenLog = listen('kernel-log', (event: any) => {
      addLog(event.payload as string);
    });

    const unlistenStoreEvent = listen('store-event-saved', (event: any) => {
      const e = event.payload;
      addLog(`[STORE] ${e.details}`);
    });

    return () => {
      unlistenTick.then(f => f());
      unlistenLog.then(f => f());
      unlistenStoreEvent.then(f => f());
    };
  }, []);

  const renderView = () => {
    switch (activeTab) {
      case 'dashboard': return <DashboardView />;
      case 'gene': return <GeneLayer />;
      case 'safeguard': return <Safeguard />;
      case 'quantum': return <QuantumModule />;
      case 'network': return <NetworkTopology />;
      case 'agents': return <AgentsList />;
      case 'swarm': return <SwarmIntelligence />;
      case 'marketplace': return <MarketplaceView />;
      case 'governance': return <Governance />;
      case 'neural': return <NeuralLink />;
      case 'vault': return <Vault />;
      default: return <DashboardView />;
    }
  };

  if (!isUnlocked) {
    return <LockScreen onUnlock={() => setIsUnlocked(true)} />;
  }

  return (
    <div className="h-screen w-screen bg-background text-cyan-neon flex overflow-hidden font-sans">
      <Sidebar />
      <div className="flex-1 flex flex-col p-6 min-w-0">
        <header className="flex justify-between items-center border-b border-cyan-neon/20 pb-4 mb-6">
          <div>
            <h1 className="text-3xl font-bold tracking-wider uppercase text-shadow-neon">NETGENE OS</h1>
            <p className="text-xs font-mono opacity-60">v1.0.0 Apex Megastructure // Desktop Terminal</p>
          </div>
          <div className="flex gap-4">
             <div className="text-right">
               <div className="text-[10px] uppercase font-mono opacity-50">Master Gene</div>
               <div className="font-mono text-sm text-fuchsia-400">df2dd4dd...b2dc</div>
             </div>
          </div>
        </header>

        <main className="flex-1 flex min-h-0 overflow-y-auto">
          {renderView()}
        </main>
      </div>
    </div>
  );
}

export default App;
// Triggering Vite HMR
