import { useRef, useMemo } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { Sphere, OrbitControls, Stars } from '@react-three/drei';
import * as THREE from 'three';
import { useStore } from '../store';

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
  const topology = useStore((state) => state.topology);

  // Place each known node on a circle; no synthetic nodes or links.
  const nodes = useMemo(() => {
    const count = topology.length;
    return topology.map((node: any, i: number) => {
      const angle = (i / Math.max(count, 1)) * Math.PI * 2;
      return {
        pos: [Math.cos(angle) * 5, Math.sin(angle) * 5, 0] as [number, number, number],
        color: node.status === 'ACTIVE' ? '#39ff14' : '#00f0ff',
      };
    });
  }, [topology]);

  return (
    <div className="w-full h-full rounded-xl overflow-hidden border border-cyan-neon/30 shadow-neon-cyan relative">
      <div className="absolute top-4 left-4 z-10 text-cyan-neon text-xs font-mono uppercase tracking-widest bg-background/80 px-2 py-1 rounded">
        Known Nodes ({nodes.length})
      </div>
      <Canvas camera={{ position: [0, 0, 15] }}>
        <color attach="background" args={['#030712']} />
        <ambientLight intensity={0.5} />
        <pointLight position={[10, 10, 10]} intensity={1} color="#ff003c" />
        <Stars radius={100} depth={50} count={5000} factor={4} saturation={0} fade speed={1} />
        
        {nodes.map((node, i) => (
          <Node key={`node-${i}`} position={node.pos} color={node.color} />
        ))}

        <OrbitControls autoRotate autoRotateSpeed={0.5} enablePan={false} />
      </Canvas>
    </div>
  );
};
