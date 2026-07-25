import React, { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Brain, Activity, Zap, ShieldAlert, CheckCircle2 } from 'lucide-react';

interface NeuralEvent {
    event_id: string;
    timestamp: string;
    focus_target: string;
    cognitive_load: number;
    raw_signals: {
        alpha: number;
        beta: number;
        gamma: number;
        signal_quality: number;
    };
    converted_action: string;
}

export function NeuralLink() {
    const [events, setEvents] = useState<NeuralEvent[]>([]);
    const [isStreaming, setIsStreaming] = useState(false);
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        let unlisten: any = null;

        const setupListener = async () => {
            unlisten = await listen<NeuralEvent>('neural-event', (event) => {
                setEvents(prev => [event.payload, ...prev].slice(0, 10));
            });
        };
        setupListener();

        return () => {
            if (unlisten) unlisten();
        };
    }, []);

    // Simulação do backend
    useEffect(() => {
        let interval: any;
        if (isStreaming) {
            interval = setInterval(() => {
                const beta = Math.random();
                const gamma = Math.random();
                invoke('stream_neural_telemetry', { beta, gamma }).catch(console.error);
            }, 2000);
        }
        return () => clearInterval(interval);
    }, [isStreaming]);

    // Desenhar ondas (visual mock)
    useEffect(() => {
        if (!canvasRef.current || !isStreaming) return;
        const ctx = canvasRef.current.getContext('2d');
        if (!ctx) return;

        let frame = 0;
        let animationId: number;

        const draw = () => {
            ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
            
            const drawWave = (color: string, offset: number, amplitude: number, frequency: number) => {
                ctx.beginPath();
                ctx.strokeStyle = color;
                ctx.lineWidth = 2;
                for (let i = 0; i < ctx.canvas.width; i++) {
                    const y = Math.sin(i * frequency + (frame * offset)) * amplitude + (ctx.canvas.height / 2);
                    if (i === 0) ctx.moveTo(i, y);
                    else ctx.lineTo(i, y);
                }
                ctx.stroke();
            };

            // Alpha, Beta, Gamma
            drawWave('#38bdf8', 0.05, 10, 0.02); // Alpha (Blue)
            drawWave('#a855f7', 0.1, 20, 0.05);  // Beta (Purple)
            drawWave('#f43f5e', 0.2, 35, 0.1);   // Gamma (Rose)

            frame++;
            animationId = requestAnimationFrame(draw);
        };

        draw();
        return () => cancelAnimationFrame(animationId);
    }, [isStreaming]);

    return (
        <div className="h-full flex flex-col p-6 overflow-y-auto animate-fade-in relative">
            <div className="flex justify-between items-start mb-6">
                <div>
                    <h1 className="text-3xl font-bold text-shadow-neon flex items-center text-rose-400">
                        <Brain className="mr-3" size={32} /> Neural BCI Adapter
                    </h1>
                    <p className="text-slate-400 mt-2 text-sm">Direct Brain-Computer Interface Telemetry</p>
                </div>
                
                <div className="flex flex-col items-end gap-2">
                    <button 
                        onClick={() => setIsStreaming(!isStreaming)}
                        className={`flex items-center gap-2 px-6 py-3 rounded-xl font-bold transition-all shadow-neon ${isStreaming ? 'bg-rose-600 hover:bg-rose-500 text-white animate-pulse-slow' : 'bg-slate-800 border border-slate-600 text-slate-300 hover:bg-slate-700'}`}
                    >
                        <Zap size={20} />
                        {isStreaming ? 'DISCONNECT BCI' : 'INITIALIZE NEURAL LINK'}
                    </button>
                    <span className="text-[10px] text-slate-500 font-mono uppercase bg-slate-900 px-2 py-1 rounded border border-rose-900/30">
                        <ShieldAlert className="inline mr-1" size={10} /> 
                        Simulation Mode Active - No physical hardware connected
                    </span>
                </div>
            </div>

            <div className="bg-black/60 border border-rose-900/50 rounded-xl p-6 mb-6 shadow-[0_0_20px_rgba(225,29,72,0.1)]">
                <div className="flex justify-between items-center mb-4">
                    <h2 className="text-sm font-mono text-rose-400 uppercase tracking-widest flex items-center gap-2">
                        <Activity size={16} /> Raw Cortex Signals
                    </h2>
                    <div className="flex gap-4 text-xs font-mono">
                        <span className="text-sky-400">Alpha 8-12Hz</span>
                        <span className="text-purple-400">Beta 12-30Hz</span>
                        <span className="text-rose-400">Gamma 30-100Hz</span>
                    </div>
                </div>
                <div className="w-full h-40 bg-slate-950 rounded-lg overflow-hidden border border-slate-800 relative">
                    {!isStreaming && (
                        <div className="absolute inset-0 flex items-center justify-center text-slate-600 font-mono text-sm uppercase">
                            Awaiting Neural Connection...
                        </div>
                    )}
                    <canvas ref={canvasRef} width={800} height={160} className="w-full h-full opacity-80" />
                </div>
            </div>

            <div className="flex-1 bg-slate-900 border border-slate-700 rounded-xl p-6 overflow-hidden flex flex-col">
                <h2 className="text-sm font-mono text-slate-300 uppercase tracking-widest mb-4">Intent Conversion Stream</h2>
                
                <div className="flex-1 overflow-y-auto pr-2 space-y-3">
                    {events.length === 0 ? (
                        <div className="text-center text-slate-500 mt-10 font-mono text-sm">No intents captured yet.</div>
                    ) : events.map((ev, i) => (
                        <div key={ev.event_id} className={`bg-black/40 border border-slate-800 p-4 rounded-lg flex items-center justify-between transition-all ${i === 0 ? 'border-rose-500/50 shadow-[0_0_15px_rgba(225,29,72,0.15)] bg-rose-950/20' : 'opacity-70'}`}>
                            <div className="flex items-center gap-4">
                                <div className={`p-2 rounded-full ${ev.cognitive_load > 0.75 ? 'bg-rose-500/20 text-rose-400' : 'bg-emerald-500/20 text-emerald-400'}`}>
                                    {ev.cognitive_load > 0.75 ? <Activity size={16} /> : <CheckCircle2 size={16} />}
                                </div>
                                <div>
                                    <p className="font-mono text-sm text-white">{ev.converted_action}</p>
                                    <p className="text-[10px] text-slate-500 font-mono">{ev.timestamp} • Target: {ev.focus_target}</p>
                                </div>
                            </div>
                            
                            <div className="text-right">
                                <p className="text-xs font-mono text-slate-400">Cognitive Load</p>
                                <p className={`text-lg font-bold ${ev.cognitive_load > 0.75 ? 'text-rose-400' : 'text-emerald-400'}`}>
                                    {(ev.cognitive_load * 100).toFixed(1)}%
                                </p>
                            </div>
                        </div>
                    ))}
                </div>
            </div>

        </div>
    );
}
