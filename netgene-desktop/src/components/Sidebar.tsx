import React, { useState } from 'react';
import { useStore } from '../store';
import { 
  LayoutDashboard, 
  Dna, 
  ShieldAlert, 
  Cpu, 
  Network, 
  Bot, 
  Globe,
  Menu,
  BrainCircuit,
  Share2,
  Settings,
  Atom,
  Landmark,
  Brain,
  Lock,
  ShoppingCart
} from 'lucide-react';

const navItems = [
  { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { id: 'gene', label: 'Gene Layer', icon: Dna },
  { id: 'safeguard', label: 'Safeguard', icon: ShieldAlert },
  { id: 'quantum', label: 'Quantum Module', icon: Cpu },
  { id: 'topology', label: 'Network Topology', icon: Share2 },
  { id: 'agents', label: 'BDI Agents', icon: Bot },
  { id: 'marketplace', label: 'Gene Market', icon: ShoppingCart },
  { id: 'vault', label: 'Personal Vault', icon: Lock },
  { id: 'governance', label: 'DAO Governance', icon: Landmark },
  { id: 'neural', label: 'Neural BCI', icon: Brain },
];

export function Sidebar() {
  const { activeTab, setActiveTab } = useStore();
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className={`flex flex-col bg-black/40 border-r border-cyan-neon/20 transition-all duration-300 ${collapsed ? 'w-16' : 'w-64'} h-full p-4`}>
      <div className="flex items-center justify-between mb-8">
        {!collapsed && <h2 className="text-cyan-neon font-bold tracking-widest text-shadow-neon uppercase">Menu</h2>}
        <button 
          onClick={() => setCollapsed(!collapsed)}
          className="p-2 hover:bg-cyan-neon/10 rounded-lg text-cyan-neon transition-colors"
        >
          <Menu size={20} />
        </button>
      </div>

      <nav className="flex flex-col gap-2">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`flex items-center gap-4 p-3 rounded-xl transition-all duration-200
                ${isActive 
                  ? 'bg-cyan-neon/20 border border-cyan-neon/50 text-cyan-neon shadow-neon' 
                  : 'hover:bg-white/5 text-gray-400 hover:text-cyan-neon'
                }`}
              title={collapsed ? item.label : undefined}
            >
              <Icon size={20} className={isActive ? 'animate-pulse' : ''} />
              {!collapsed && (
                <span className="font-mono text-sm whitespace-nowrap">{item.label}</span>
              )}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
