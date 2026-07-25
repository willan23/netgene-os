import { create } from 'zustand';

interface NetGeneState {
  health: string;
  uptime: number;
  latency: number;
  anomalies: number;
  logs: string[];
  
  // New States
  activeTab: string;
  agents: any[];
  nodes: any[];
  quantum: any;
  safeguard: any;
  topology: any[];

  setMetrics: (metrics: Partial<NetGeneState>) => void;
  addLog: (log: string) => void;
  setLogs: (logs: string[]) => void;
  setActiveTab: (tab: string) => void;
  setAgents: (agents: any[]) => void;
  setQuantum: (quantum: any) => void;
  setSafeguard: (safeguard: any) => void;
  setTopology: (topology: any[]) => void;
}

export const useStore = create<NetGeneState>((set) => ({
  health: 'OFFLINE',
  uptime: 0,
  latency: 0,
  anomalies: 0,
  logs: ['[SYSTEM] NetGene Desktop initializing...'],
  
  activeTab: 'dashboard',
  agents: [],
  nodes: [],
  quantum: null,
  safeguard: null,
  topology: [],

  setMetrics: (metrics) => set((state) => ({ ...state, ...metrics })),
  addLog: (log) => set((state) => ({ logs: [log, ...state.logs].slice(0, 100) })),
  setLogs: (logs) => set(() => ({ logs })),
  setActiveTab: (activeTab) => set(() => ({ activeTab })),
  setAgents: (agents) => set(() => ({ agents })),
  setQuantum: (quantum) => set(() => ({ quantum })),
  setSafeguard: (safeguard) => set(() => ({ safeguard })),
  setTopology: (topology) => set(() => ({ topology })),
}));
