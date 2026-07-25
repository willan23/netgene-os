import React, { useEffect, useRef } from 'react';
import { useStore } from '../store';
import { Terminal } from 'lucide-react';

export const LogsPanel = () => {
  const { logs } = useStore();
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  return (
    <div className="bg-surface/80 border border-cyan-neon/30 rounded-xl flex flex-col h-full overflow-hidden shadow-neon-cyan backdrop-blur-md">
      <div className="bg-background/80 px-4 py-2 border-b border-cyan-neon/20 flex items-center gap-2">
        <Terminal size={16} className="text-cyan-neon" />
        <span className="text-xs font-mono text-cyan-neon uppercase">Live Kernel Logs</span>
      </div>
      <div className="flex-1 p-4 overflow-y-auto font-mono text-xs flex flex-col-reverse">
        <div ref={endRef} />
        {logs.map((log, i) => (
          <div key={i} className={`mb-1 ${log.includes('error') ? 'text-magenta-neon' : log.includes('heal') ? 'text-green-neon' : 'text-cyan-neon/80'}`}>
            <span className="opacity-50 mr-2">[{new Date().toLocaleTimeString()}]</span>
            {log}
          </div>
        ))}
      </div>
    </div>
  );
};
