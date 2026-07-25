import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Lock, File, Download, UploadCloud, ShieldCheck } from 'lucide-react';

interface VaultFile {
    file_id: string;
    filename: string;
    size_bytes: number;
    chunk_ids: string[];
    created_at: number;
}

export function Vault() {
    const [files, setFiles] = useState<VaultFile[]>([]);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        loadFiles();
    }, []);

    const loadFiles = async () => {
        try {
            const result: VaultFile[] = await invoke('vault_list_files');
            setFiles(result);
        } catch (e) {
            console.error(e);
        }
    };

    const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.files || e.target.files.length === 0) return;
        
        const file = e.target.files[0];
        setLoading(true);

        try {
            const buffer = await file.arrayBuffer();
            const data = Array.from(new Uint8Array(buffer));
            
            await invoke('vault_store_file', { 
                filename: file.name,
                data: data
            });
            
            loadFiles();
        } catch (err) {
            console.error(err);
        } finally {
            setLoading(false);
        }
    };

    const handleDownload = async (file_id: string, originalName: string) => {
        try {
            const [filename, data]: [string, number[]] = await invoke('vault_retrieve_file', { fileId: file_id });
            const uint8Array = new Uint8Array(data);
            const blob = new Blob([uint8Array]);
            
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.style.display = 'none';
            a.href = url;
            a.download = filename || originalName;
            document.body.appendChild(a);
            a.click();
            window.URL.revokeObjectURL(url);
            document.body.removeChild(a);
        } catch (err) {
            console.error(err);
        }
    };

    const formatBytes = (bytes: number) => {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };

    return (
        <div className="h-full flex flex-col p-6 overflow-y-auto animate-fade-in">
            <div className="flex justify-between items-start mb-8">
                <div>
                    <h1 className="text-3xl font-bold text-shadow-neon flex items-center text-cyan-400">
                        <Lock className="mr-3" size={32} /> Personal Vault
                    </h1>
                    <p className="text-slate-400 mt-2 text-sm">P2P Encrypted File System (Gene Cryptography)</p>
                </div>
                <div>
                    <label className="bg-cyan-700 hover:bg-cyan-600 cursor-pointer text-white px-6 py-3 rounded-lg font-bold transition-colors flex items-center gap-2 shadow-neon">
                        {loading ? <div className="animate-spin h-5 w-5 border-2 border-white border-t-transparent rounded-full" /> : <UploadCloud />}
                        {loading ? 'Encrypting...' : 'Upload File'}
                        <input type="file" className="hidden" onChange={handleFileUpload} disabled={loading} />
                    </label>
                </div>
            </div>

            <div className="bg-slate-900 border border-slate-700 rounded-xl overflow-hidden shadow-neon">
                <table className="w-full text-left">
                    <thead className="bg-black/50 text-cyan-500 font-mono text-xs uppercase tracking-wider">
                        <tr>
                            <th className="p-4">File Name</th>
                            <th className="p-4">Size</th>
                            <th className="p-4">Chunks</th>
                            <th className="p-4">Security</th>
                            <th className="p-4">Actions</th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-800 text-slate-300 text-sm">
                        {files.length === 0 ? (
                            <tr>
                                <td colSpan={5} className="p-8 text-center text-slate-500 font-mono">
                                    No files in the vault. Upload a file to encrypt and chunk it.
                                </td>
                            </tr>
                        ) : (
                            files.map(f => (
                                <tr key={f.file_id} className="hover:bg-slate-800/50 transition-colors">
                                    <td className="p-4 flex items-center gap-3">
                                        <File className="text-cyan-600" />
                                        <span className="font-semibold">{f.filename}</span>
                                    </td>
                                    <td className="p-4 font-mono text-slate-400">{formatBytes(f.size_bytes)}</td>
                                    <td className="p-4 font-mono">
                                        <span className="bg-slate-800 px-2 py-1 rounded text-xs">{f.chunk_ids.length} chunks</span>
                                    </td>
                                    <td className="p-4">
                                        <span className="flex items-center gap-1 text-emerald-500 text-xs font-mono uppercase bg-emerald-900/20 px-2 py-1 rounded border border-emerald-900 w-fit">
                                            <ShieldCheck size={14} /> AES-256-GCM
                                        </span>
                                    </td>
                                    <td className="p-4">
                                        <button 
                                            onClick={() => handleDownload(f.file_id, f.filename)}
                                            className="text-cyan-500 hover:text-cyan-400 p-2 rounded-lg hover:bg-cyan-900/30 transition-colors"
                                            title="Download & Decrypt"
                                        >
                                            <Download size={18} />
                                        </button>
                                    </td>
                                </tr>
                            ))
                        )}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
