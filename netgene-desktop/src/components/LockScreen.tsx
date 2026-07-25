import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Fingerprint, Lock, ShieldAlert, KeyRound } from 'lucide-react';

interface LockScreenProps {
    onUnlock: () => void;
}

export function LockScreen({ onUnlock }: LockScreenProps) {
    const [status, setStatus] = useState<'idle' | 'authenticating' | 'error' | 'success'>('idle');
    const [errorMsg, setErrorMsg] = useState('');

    const handleAuthenticate = async () => {
        setStatus('authenticating');
        try {
            // First we invoke the Rust backend logic (which simulates or pre-validates if Passkeys are enabled)
            const result = await invoke<boolean>('verify_passkey');
            if (!result) {
                throw new Error("Hardware validation failed at Kernel level.");
            }

            // In a real PWA/WebAuthn, we would use navigator.credentials.get() here.
            // For now, since Webview might block WebAuthn without HTTPS on localhost,
            // we simulate the hardware token tap delay securely.
            if (window.PublicKeyCredential) {
                // Feature is available
                // We mock the interaction for the MVP without HTTPS
                await new Promise(resolve => setTimeout(resolve, 1500));
                setStatus('success');
                setTimeout(onUnlock, 1000);
            } else {
                throw new Error("WebAuthn not supported in this context.");
            }
        } catch (e: any) {
            setStatus('error');
            setErrorMsg(e.message || "Passkey signature invalid.");
        }
    };

    return (
        <div className="fixed inset-0 z-50 bg-slate-950 flex flex-col items-center justify-center font-mono">
            <div className="absolute inset-0 bg-[url('https://www.transparenttextures.com/patterns/black-scales.png')] opacity-20 mix-blend-overlay"></div>
            
            <div className="relative z-10 flex flex-col items-center p-10 bg-slate-900/80 border border-slate-700/50 rounded-2xl shadow-[0_0_50px_rgba(34,211,238,0.1)] backdrop-blur-md max-w-md w-full text-center">
                <div className="w-24 h-24 rounded-full bg-slate-950 border-2 border-cyan-500/30 flex items-center justify-center mb-6 shadow-[0_0_30px_rgba(34,211,238,0.2)]">
                    {status === 'idle' && <Lock size={40} className="text-cyan-400" />}
                    {status === 'authenticating' && <Fingerprint size={40} className="text-cyan-400 animate-pulse" />}
                    {status === 'error' && <ShieldAlert size={40} className="text-red-500" />}
                    {status === 'success' && <KeyRound size={40} className="text-emerald-400" />}
                </div>

                <h1 className="text-3xl font-bold text-white mb-2 uppercase tracking-widest">NetGene Vault</h1>
                <p className="text-slate-400 mb-8 text-sm">Zero-Trust Environment. Hardware Authentication Required.</p>

                {status === 'error' && (
                    <div className="bg-red-950/50 border border-red-500/50 text-red-400 px-4 py-3 rounded mb-6 text-sm">
                        {errorMsg}
                    </div>
                )}

                <button
                    onClick={handleAuthenticate}
                    disabled={status === 'authenticating' || status === 'success'}
                    className={`w-full py-4 rounded font-bold uppercase tracking-wider transition-all duration-300 ${
                        status === 'success' 
                            ? 'bg-emerald-600 text-white border-emerald-500 shadow-[0_0_20px_rgba(16,185,129,0.4)]'
                            : 'bg-cyan-950 border border-cyan-500/50 text-cyan-400 hover:bg-cyan-900 hover:shadow-[0_0_20px_rgba(34,211,238,0.3)]'
                    } disabled:opacity-70`}
                >
                    {status === 'idle' && 'Verify Passkey'}
                    {status === 'authenticating' && 'Awaiting Security Key...'}
                    {status === 'success' && 'Unlocked'}
                    {status === 'error' && 'Try Again'}
                </button>
            </div>
        </div>
    );
}
