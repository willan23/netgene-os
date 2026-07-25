import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Sphere, Line, OrbitControls, Stars } from '@react-three/drei';
import * as THREE from 'three';

const Node = ({ position, color }: { position: [number, number, number], color: string }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  
  useFrame((state) => {
    if (meshRef.current) {
      meshRef.current.position.y += Math.sin(state.clock.elapsedTime * 2 + position[0]) * 0.005;
    }
  });

  return (
    <Sphere ref={meshRef} args={[0.2, 16, 16]} position={position}>
      <meshStandardMaterial color={color} emissive={color} emissiveIntensity={2} toneMapped={false} />
    </Sphere>
  );
};

export const Megastructure3D = () => {
  // Generate random nodes
  const nodes = useMemo(() => {
    return Array.from({ length: 15 }).map(() => ({
      pos: [(Math.random() - 0.5) * 10, (Math.random() - 0.5) * 10, (Math.random() - 0.5) * 10] as [number, number, number],
      color: Math.random() > 0.8 ? '#39ff14' : '#00f0ff', // green or cyan
    }));
  }, []);

  // Generate lines between nodes
  const lines = useMemo(() => {
    const l = [];
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        if (Math.random() > 0.7) {
          l.push([nodes[i].pos, nodes[j].pos]);
        }
      }
    }
    return l;
  }, [nodes]);

  return (
    <div className="w-full h-full rounded-xl overflow-hidden border border-cyan-neon/30 shadow-neon-cyan relative">
      <div className="absolute top-4 left-4 z-10 text-cyan-neon text-xs font-mono uppercase tracking-widest bg-background/80 px-2 py-1 rounded">
        Quantum Mesh Topology
      </div>
      <Canvas camera={{ position: [0, 0, 15] }}>
        <color attach="background" args={['#030712']} />
        <ambientLight intensity={0.5} />
        <pointLight position={[10, 10, 10]} intensity={1} color="#ff003c" />
        <Stars radius={100} depth={50} count={5000} factor={4} saturation={0} fade speed={1} />
        
        {nodes.map((node, i) => (
          <Node key={`node-${i}`} position={node.pos} color={node.color} />
        ))}

        {lines.map((pts, i) => (
          <Line key={`line-${i}`} points={pts as any} color="#00f0ff" opacity={0.2} transparent lineWidth={1} />
        ))}
        
        <OrbitControls autoRotate autoRotateSpeed={0.5} enablePan={false} />
      </Canvas>
    </div>
  );
};
