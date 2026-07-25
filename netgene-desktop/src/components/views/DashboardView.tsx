import { DashboardCards } from '../DashboardCards';
import { Megastructure3D } from '../Megastructure3D';
import { IntentTerminal } from '../IntentTerminal';
import { LogsPanel } from '../LogsPanel';

export function DashboardView() {
  return (
    <div className="flex-1 grid grid-cols-12 grid-rows-6 gap-6 min-h-0">
      {/* Top Cards (Span full width) */}
      <div className="col-span-12 row-span-1">
        <DashboardCards />
      </div>

      {/* 3D Visualization */}
      <div className="col-span-8 row-span-5 flex flex-col gap-4">
        <div className="flex-1 relative border border-cyan-neon/30 rounded-xl overflow-hidden bg-black/50 shadow-[0_0_25px_rgba(0,255,255,0.1)]">
          <Megastructure3D />
        </div>
        <div className="flex-none">
          <IntentTerminal />
        </div>
      </div>

      {/* Logs and Details */}
      <div className="col-span-4 row-span-5 flex flex-col gap-6">
         <LogsPanel />
      </div>
    </div>
  );
}
