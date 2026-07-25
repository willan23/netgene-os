import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, Fingerprint, Key, Dna, Lock, Unlock, Plus } from 'lucide-react';

interface Keypair {
    public: { n: any; n_sq: any; g: any };
    private: { lambda: any; mu: any };
}

export function GeneLayer() {
  const [keys, setKeys] = useState<Keypair | null>(null);
  
  const [valA, setValA] = useState<number>(15);
  const [valB, setValB] = useState<number>(25);
  
  const [cipherA, setCipherA] = useState<string>('');
  const [cipherB, setCipherB] = useState<string>('');
  const [cipherSum, setCipherSum] = useState<string>('');
  const [decryptedSum, setDecryptedSum] = useState<number | null>(null);

  const [loading, setLoading] = useState(false);

  const generateKeys = async () => {
      setLoading(true);
      try {
          const kp: Keypair = await invoke('generate_fhe_keys');
          setKeys(kp);
          // Reset states
          setCipherA('');
          setCipherB('');
          setCipherSum('');
          setDecryptedSum(null);
      } catch(e) {
          console.error(e);
      }
      setLoading(false);
  };

  const encryptValues = async () => {
      if(!keys) return;
      setLoading(true);
      try {
          const cA: string = await invoke('fhe_encrypt', { pubKey: keys.public, m: valA });
          const cB: string = await invoke('fhe_encrypt', { pubKey: keys.public, m: valB });
          setCipherA(cA);
          setCipherB(cB);
          setCipherSum('');
          setDecryptedSum(null);
      } catch(e) {
          console.error(e);
      }
      setLoading(false);
  };

  const computeSum = async () => {
      if(!keys || !cipherA || !cipherB) return;
      setLoading(true);
      try {
          const cSum: string = await invoke('fhe_homomorphic_add', { pubKey: keys.public, c1: cipherA, c2: cipherB });
          setCipherSum(cSum);
          setDecryptedSum(null);
      } catch(e) {
          console.error(e);
      }
      setLoading(false);
  };

  const decryptResult = async () => {
      if(!keys || !cipherSum) return;
      setLoading(true);
      try {
          const res: number = await invoke('fhe_decrypt', { pubKey: keys.public, privKey: keys.private, c: cipherSum });
          setDecryptedSum(res);
      } catch(e) {
          console.error(e);
      }
      setLoading(false);
  };

  return (
    <div className="flex-1 flex flex-col gap-6 w-full h-full animate-fade-in overflow-y-auto">
      <h2 className="text-2xl font-bold uppercase tracking-widest text-shadow-neon flex items-center gap-3">
        <Dna className="text-cyan-neon" />
        Gene Cryptography Layer (FHE)
      </h2>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="border border-cyan-neon/30 bg-black/50 p-6 rounded-xl flex flex-col gap-6 shadow-neon">
          <h3 className="text-sm font-mono opacity-60 uppercase border-b border-cyan-neon/20 pb-2">Master Gene Info</h3>
          
          <div className="flex items-center gap-4">
            <ShieldCheck size={40} className="text-emerald-400" />
            <div>
              <p className="text-xs font-mono opacity-50">Homomorphic Engine</p>
              <p className="font-bold text-lg text-emerald-400">PAILLIER SYSTEM ACTIVE</p>
            </div>
          </div>

          <div className="flex items-center gap-4">
            <Key size={40} className="text-cyan-neon" />
            <div className="w-full">
              <p className="text-xs font-mono opacity-50">Keypair Generation</p>
              <button 
                onClick={generateKeys}
                disabled={loading}
                className="mt-2 bg-cyan-900/50 hover:bg-cyan-800 text-cyan-300 border border-cyan-500/50 px-4 py-2 rounded text-sm w-full transition-colors"
              >
                {keys ? "Regenerate Keys" : "Generate Paillier Keys"}
              </button>
            </div>
          </div>
          
          {keys && (
              <div className="font-mono bg-black/80 p-3 rounded text-[10px] border border-cyan-neon/20 break-all text-fuchsia-300 max-h-32 overflow-y-auto">
                  <span className="text-cyan-500">Public Key (N):</span> {keys.public.n.toString()} <br/><br/>
                  <span className="text-cyan-500">Private Key (Lambda):</span> {keys.private.lambda.toString()}
              </div>
          )}
        </div>

        <div className="border border-fuchsia-500/30 bg-black/50 p-6 rounded-xl flex flex-col gap-4 shadow-[0_0_15px_rgba(217,70,239,0.1)]">
          <h3 className="text-sm font-mono opacity-60 uppercase border-b border-fuchsia-500/20 pb-2 text-fuchsia-400">Homomorphic Compute Sandbox</h3>
          
          <p className="text-xs text-gray-400">Demonstrating Zero-Trust Routing: The Kernel adds encrypted latencies without ever seeing the real values.</p>

          <div className="grid grid-cols-2 gap-4">
              <div>
                  <label className="text-xs font-mono text-cyan-400">Route A Latency (ms)</label>
                  <input type="number" value={valA} onChange={e=>setValA(Number(e.target.value))} className="w-full bg-slate-900 border border-slate-700 rounded p-2 text-white font-mono mt-1" />
              </div>
              <div>
                  <label className="text-xs font-mono text-cyan-400">Route B Latency (ms)</label>
                  <input type="number" value={valB} onChange={e=>setValB(Number(e.target.value))} className="w-full bg-slate-900 border border-slate-700 rounded p-2 text-white font-mono mt-1" />
              </div>
          </div>

          <button onClick={encryptValues} disabled={!keys || loading} className="bg-fuchsia-900/50 hover:bg-fuchsia-800 text-fuchsia-300 border border-fuchsia-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center justify-center gap-2 disabled:opacity-50">
              <Lock size={16} /> Encrypt Values
          </button>

          {cipherA && (
              <div className="grid grid-cols-2 gap-2 text-[10px] font-mono break-all text-gray-500">
                  <div className="bg-slate-900 p-2 rounded">Cipher A: {cipherA.substring(0,20)}...</div>
                  <div className="bg-slate-900 p-2 rounded">Cipher B: {cipherB.substring(0,20)}...</div>
              </div>
          )}

          <button onClick={computeSum} disabled={!cipherA || loading} className="bg-blue-900/50 hover:bg-blue-800 text-blue-300 border border-blue-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center justify-center gap-2 disabled:opacity-50">
              <Plus size={16} /> Homomorphic Addition (Kernel Logic)
          </button>

          {cipherSum && (
              <div className="text-[10px] font-mono break-all text-blue-400 bg-slate-900 p-2 rounded">
                  Cipher Sum: {cipherSum.substring(0,40)}...
              </div>
          )}

          <button onClick={decryptResult} disabled={!cipherSum || loading} className="bg-emerald-900/50 hover:bg-emerald-800 text-emerald-300 border border-emerald-500/50 px-4 py-2 rounded text-sm transition-colors flex items-center justify-center gap-2 disabled:opacity-50">
              <Unlock size={16} /> Decrypt Result (Node Logic)
          </button>

          {decryptedSum !== null && (
              <div className="text-center p-4 bg-emerald-950/30 border border-emerald-500/30 rounded-lg">
                  <p className="text-xs font-mono text-emerald-500 mb-1">True Plaintext Sum</p>
                  <p className="text-3xl font-bold text-emerald-400">{decryptedSum} ms</p>
              </div>
          )}
        </div>
      </div>
    </div>
  );
}
