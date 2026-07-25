import React from 'react';
import { useStore } from '../store';
import { Activity, Zap, ShieldAlert, Cpu } from 'lucide-react';

export const DashboardCards = () => {
  const { health, uptime, latency, anomalies } = useStore();

  const cards = [
    { title: 'Network Status', value: health, icon: Activity, color: 'text-green-neon' },
    { title: 'Latency (avg)', value: `${latency} ms`, icon: Zap, color: 'text-cyan-neon' },
    { title: 'Anomalies', value: anomalies.toString(), icon: ShieldAlert, color: anomalies > 0 ? 'text-magenta-neon' : 'text-cyan-neon' },
    { title: 'Uptime', value: `${uptime}%`, icon: Cpu, color: 'text-green-neon' },
  ];

  return (
    <div className="grid grid-cols-4 gap-4">
      {cards.map((c, i) => (
        <div key={i} className="bg-surface/50 border border-cyan-neon/20 p-4 rounded-xl flex items-center gap-4 backdrop-blur-sm transition-all hover:border-cyan-neon/60 hover:shadow-neon-cyan">
          <div className={`p-3 bg-background rounded-lg ${c.color}`}>
            <c.icon size={24} />
          </div>
          <div>
            <div className="text-xs text-gray-400 font-mono uppercase">{c.title}</div>
            <div className={`text-xl font-bold font-mono ${c.color}`}>{c.value}</div>
          </div>
        </div>
      ))}
    </div>
  );
};
