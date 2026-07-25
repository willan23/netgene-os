// ═══════════════════════════════════════════════════════════
//   NetGene OS — Dashboard JS
//   Live metrics, 3D network visualization, tab routing
// ═══════════════════════════════════════════════════════════

// ─── State ───────────────────────────────────────────────
const state = {
  startTime: Date.now(),
  geneId: crypto.randomUUID(),
  geneFp: generateFp(),
  nodes: generateNodes(8),
  anomalies: 0,
  heals: 0,
  networkHealth: 96.2,
  quantumGain: 18.4,
  logs: [],
  tick: 0,
};

// ─── Helpers ─────────────────────────────────────────────
function generateFp() {
  const chars = '0123456789abcdef';
  return Array.from({length: 16}, () => chars[Math.floor(Math.random() * 16)]).join('');
}

function generateNodes(count) {
  return Array.from({length: count}, (_, i) => ({
    id: `node-${String(i).padStart(2,'0')}`,
    status: i === 3 ? 'degraded' : 'active',
    load: 0.2 + i * 0.08,
    latency: 5 + i * 3.5,
    connections: 2 + i % 4,
    x: 0, y: 0, // set by canvas
    vx: (Math.random() - 0.5) * 0.3,
    vy: (Math.random() - 0.5) * 0.3,
  }));
}

function formatUptime() {
  const s = Math.floor((Date.now() - state.startTime) / 1000);
  return `${String(Math.floor(s/3600)).padStart(2,'0')}:${String(Math.floor((s%3600)/60)).padStart(2,'0')}:${String(s%60).padStart(2,'0')}`;
}

function log(msg, type = 'info') {
  const now = new Date().toTimeString().slice(0,8);
  state.logs.unshift({ msg: `[${now}] ${msg}`, type });
  if (state.logs.length > 100) state.logs.pop();
  renderLogs();
}

// ─── Init ─────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  // Initial gene data
  document.getElementById('gene-fp').textContent = state.geneFp;
  document.getElementById('gene-fp-panel').textContent = state.geneFp;
  document.getElementById('g-id').textContent = state.geneId;
  document.getElementById('g-fp').textContent = state.geneFp;
  document.getElementById('g-created').textContent = new Date().toISOString();

  // Capabilities
  const caps = ['gene.create','gene.revoke','node.spawn','agent.spawn','network.admin','quantum.run'];
  const capsGrid = document.getElementById('caps-grid');
  caps.forEach(c => {
    const el = document.createElement('div');
    el.className = 'cap-pill'; el.textContent = c;
    capsGrid.appendChild(el);
  });

  // Nodes table
  renderNodesTable();

  // Background canvas
  initBgCanvas();

  // Network canvas
  initNetworkCanvas('network-canvas');
  initNetworkCanvas('network-canvas-2');

  // Nav tabs
  document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', e => {
      e.preventDefault();
      const tab = item.dataset.tab;
      document.querySelectorAll('.nav-item').forEach(i => i.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
      item.classList.add('active');
      document.getElementById(`tab-${tab}`).classList.add('active');
      document.getElementById('page-title').textContent = item.textContent.trim().replace(/^[^\s]+\s+/, '');
    });
  });

  // Boot log
  log('🧬 NetGene OS v0.1 booting...', 'ok');
  setTimeout(() => log('✅ Gene Layer initialized — fp:' + state.geneFp, 'ok'), 400);
  setTimeout(() => log('✅ Netsphere Kernel online (3 agents)', 'ok'), 800);
  setTimeout(() => log('✅ Quantum Module ready (QAOA-sim + SQA)', 'ok'), 1200);
  setTimeout(() => log('✅ Safeguard Layer armed', 'ok'), 1600);
  setTimeout(() => log('🟢 System ONLINE — All layers active', 'ok'), 2000);

  // Start tick
  setInterval(tick, 100);
  setInterval(updateHeader, 1000);
});

// ─── Tick ─────────────────────────────────────────────────
function tick() {
  state.tick++;

  // Jitter node metrics
  if (state.tick % 10 === 0) {
    state.nodes.forEach((n, i) => {
      const j = Math.sin(state.tick * 0.017 + i) * 0.02;
      n.load = Math.min(0.95, Math.max(0.05, n.load + j));
      n.latency = Math.min(200, Math.max(1, n.latency + j * 50));
    });
  }

  // Occasional anomaly
  if (state.tick % 500 === 0) {
    state.anomalies++;
    state.heals++;
    const node = state.nodes[Math.floor(Math.random() * state.nodes.length)];
    node.status = 'degraded';
    log(`🔴 Anomaly on ${node.id} — z-score 3.2`, 'warn');
    setTimeout(() => {
      node.status = 'active';
      log(`✅ Self-heal applied on ${node.id} — rerouted`, 'ok');
    }, 3000);
  }

  // Quantum gain oscillation
  state.quantumGain = 15 + Math.sin(state.tick * 0.03) * 8;

  // Network health
  state.networkHealth = 94 + Math.sin(state.tick * 0.02) * 2.5;

  // Update KPIs
  const kpiH = document.getElementById('kpi-health');
  const kpiQ = document.getElementById('kpi-quantum');
  const kpiHl = document.getElementById('kpi-heals');
  if (kpiH) kpiH.textContent = state.networkHealth.toFixed(1) + '%';
  if (kpiQ) kpiQ.textContent = '+' + state.quantumGain.toFixed(1) + '%';
  if (kpiHl) kpiHl.textContent = state.heals;

  // Quantum panel
  const qd = document.getElementById('q-delta');
  const qd2 = document.getElementById('q-gain2');
  const qbar = document.getElementById('q-bar');
  if (qd) qd.textContent = '+' + state.quantumGain.toFixed(1) + '%';
  if (qd2) qd2.textContent = '+' + state.quantumGain.toFixed(1) + '%';
  if (qbar) qbar.style.width = (state.quantumGain * 3) + '%';

  // Safeguard
  const sga = document.getElementById('sg-anomalies');
  const sgh = document.getElementById('sg-heals');
  if (sga) sga.textContent = state.anomalies;
  if (sgh) sgh.textContent = state.heals;

  // Table
  if (state.tick % 20 === 0) renderNodesTable();
}

function updateHeader() {
  document.getElementById('uptime').textContent = formatUptime();
}

// ─── Nodes table ──────────────────────────────────────────
function renderNodesTable() {
  const tbody = document.getElementById('nodes-tbody');
  if (!tbody) return;
  tbody.innerHTML = state.nodes.map(n => {
    const statusColor = n.status === 'active' ? 'green' : n.status === 'degraded' ? 'orange' : 'red';
    const loadPct = (n.load * 100).toFixed(0);
    return `
      <tr>
        <td class="cyan mono">${n.id}</td>
        <td><span class="badge ${statusColor}">${n.status.toUpperCase()}</span></td>
        <td style="color:${n.load > 0.8 ? 'var(--red)' : 'var(--text)'}">${loadPct}%</td>
        <td style="color:var(--orange)">${n.latency.toFixed(1)}ms</td>
        <td>${n.connections}</td>
      </tr>`;
  }).join('');
}

// ─── Logs ─────────────────────────────────────────────────
function renderLogs() {
  const el = document.getElementById('event-log');
  if (!el) return;
  el.innerHTML = state.logs.slice(0, 30).map(l =>
    `<div class="log-entry ${l.type}">${l.msg}</div>`
  ).join('');
}

// ─── Background canvas ────────────────────────────────────
function initBgCanvas() {
  const canvas = document.getElementById('bg-canvas');
  const ctx = canvas.getContext('2d');
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;

  const particles = Array.from({length: 60}, () => ({
    x: Math.random() * canvas.width,
    y: Math.random() * canvas.height,
    r: Math.random() * 1.5 + 0.3,
    vx: (Math.random() - 0.5) * 0.2,
    vy: (Math.random() - 0.5) * 0.2,
    a: Math.random(),
  }));

  function drawBg() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    particles.forEach(p => {
      p.x += p.vx; p.y += p.vy;
      if (p.x < 0) p.x = canvas.width;
      if (p.x > canvas.width) p.x = 0;
      if (p.y < 0) p.y = canvas.height;
      if (p.y > canvas.height) p.y = 0;

      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r, 0, Math.PI*2);
      ctx.fillStyle = `rgba(0,255,230,${p.a * 0.3})`;
      ctx.fill();
    });

    // Connect nearby particles
    for (let i = 0; i < particles.length; i++) {
      for (let j = i+1; j < particles.length; j++) {
        const dx = particles[i].x - particles[j].x;
        const dy = particles[i].y - particles[j].y;
        const d = Math.sqrt(dx*dx + dy*dy);
        if (d < 120) {
          ctx.beginPath();
          ctx.moveTo(particles[i].x, particles[i].y);
          ctx.lineTo(particles[j].x, particles[j].y);
          ctx.strokeStyle = `rgba(0,255,230,${(1 - d/120) * 0.05})`;
          ctx.stroke();
        }
      }
    }

    requestAnimationFrame(drawBg);
  }
  drawBg();

  window.addEventListener('resize', () => {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  });
}

// ─── Network 3D Megastructure canvas ────────────────────────
function initNetworkCanvas(id) {
  const canvas = document.getElementById(id);
  if (!canvas) return;

  // Use Three.js WebGL if available
  if (typeof THREE !== 'undefined') {
    try {
      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(60, canvas.width / canvas.height, 0.1, 1000);
      const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
      renderer.setSize(canvas.width, canvas.height);

      // Create megastructure core sphere
      const coreGeo = new THREE.IcosahedronGeometry(2, 2);
      const coreMat = new THREE.MeshBasicMaterial({ color: 0x00ffe6, wireframe: true });
      const coreMesh = new THREE.Mesh(coreGeo, coreMat);
      scene.add(coreMesh);

      // Create surrounding node spheres
      const nodeGroup = new THREE.Group();
      const numNodes = state.nodes.length;
      const radius = 6;
      const nodeMeshes = [];

      for (let i = 0; i < numNodes; i++) {
        const phi = Math.acos(-1 + (2 * i) / numNodes);
        const theta = Math.sqrt(numNodes * Math.PI) * phi;
        const x = radius * Math.cos(theta) * Math.sin(phi);
        const y = radius * Math.sin(theta) * Math.sin(phi);
        const z = radius * Math.cos(phi);

        const nodeGeo = new THREE.SphereGeometry(0.4, 16, 16);
        const nodeMat = new THREE.MeshBasicMaterial({
          color: i === 3 ? 0xffa01e : 0x00ffe6,
        });
        const nodeMesh = new THREE.Mesh(nodeGeo, nodeMat);
        nodeMesh.position.set(x, y, z);
        nodeGroup.add(nodeMesh);
        nodeMeshes.push(nodeMesh);

        // Connection line to core
        const lineMat = new THREE.LineBasicMaterial({ color: 0x00ffe6, transparent: true, opacity: 0.3 });
        const lineGeo = new THREE.BufferGeometry().setFromPoints([
          new THREE.Vector3(0, 0, 0),
          new THREE.Vector3(x, y, z)
        ]);
        const line = new THREE.Line(lineGeo, lineMat);
        scene.add(line);
      }
      scene.add(nodeGroup);

      camera.position.z = 14;

      function animateThree() {
        requestAnimationFrame(animateThree);
        coreMesh.rotation.x += 0.005;
        coreMesh.rotation.y += 0.008;
        nodeGroup.rotation.y += 0.003;
        renderer.render(scene, camera);
      }
      animateThree();
      return;
    } catch (e) {
      console.warn("Three.js initialization fallback:", e);
    }
  }

  // 2D Canvas Fallback
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height;

  // Position nodes in 3D-ish layout
  const angle = (2 * Math.PI) / state.nodes.length;
  const cx = W/2, cy = H/2, rx = W * 0.35, ry = H * 0.35;
  state.nodes.forEach((n, i) => {
    n.x = cx + rx * Math.cos(angle * i);
    n.y = cy + ry * Math.sin(angle * i);
  });

  // Edges (ring + random extras)
  const edges = state.nodes.map((n, i) => ({
    from: i, to: (i+1) % state.nodes.length
  }));
  for (let i = 0; i < 4; i++) {
    const a = Math.floor(Math.random() * state.nodes.length);
    const b = Math.floor(Math.random() * state.nodes.length);
    if (a !== b) edges.push({ from: a, to: b });
  }

  let frame = 0;

  function drawNetwork() {
    ctx.clearRect(0, 0, W, H);
    frame++;

    // Slightly animate nodes
    state.nodes.forEach(n => {
      n.x += Math.sin(frame * 0.01 + n.y * 0.01) * 0.2;
      n.y += Math.cos(frame * 0.008 + n.x * 0.01) * 0.2;
    });

    // Draw edges
    edges.forEach(e => {
      const a = state.nodes[e.from], b = state.nodes[e.to];
      const active = a.status === 'active' && b.status === 'active';
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.strokeStyle = active ? 'rgba(0,255,230,0.2)' : 'rgba(255,160,30,0.15)';
      ctx.lineWidth = 1;
      ctx.stroke();

      // Animated data packet
      const t = (frame * 0.01 + e.from * 0.3) % 1;
      const px = a.x + (b.x - a.x) * t;
      const py = a.y + (b.y - a.y) * t;
      ctx.beginPath();
      ctx.arc(px, py, 2, 0, Math.PI*2);
      ctx.fillStyle = active ? 'rgba(0,255,230,0.7)' : 'rgba(255,160,30,0.5)';
      ctx.fill();
    });

    // Draw nodes
    state.nodes.forEach(n => {
      const color = n.status === 'active' ? '#00ffe6' : '#ffa01e';
      const glowColor = n.status === 'active' ? 'rgba(0,255,230,0.3)' : 'rgba(255,160,30,0.3)';
      const r = 6 + n.load * 4;

      // Glow
      const grad = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, r * 2.5);
      grad.addColorStop(0, glowColor);
      grad.addColorStop(1, 'transparent');
      ctx.beginPath();
      ctx.arc(n.x, n.y, r * 2.5, 0, Math.PI*2);
      ctx.fillStyle = grad;
      ctx.fill();

      // Node circle
      ctx.beginPath();
      ctx.arc(n.x, n.y, r, 0, Math.PI*2);
      ctx.fillStyle = color;
      ctx.fill();

      // Load ring
      ctx.beginPath();
      ctx.arc(n.x, n.y, r + 3, -Math.PI/2, -Math.PI/2 + n.load * 2 * Math.PI);
      ctx.strokeStyle = n.load > 0.8 ? '#ff3c50' : '#32ff78';
      ctx.lineWidth = 2;
      ctx.stroke();

      // Label
      ctx.fillStyle = 'rgba(226,232,240,0.8)';
      ctx.font = '9px JetBrains Mono, monospace';
      ctx.textAlign = 'center';
      ctx.fillText(n.id, n.x, n.y + r + 14);
    });

    // Title overlay
    ctx.fillStyle = 'rgba(0,255,230,0.4)';
    ctx.font = '10px Inter, sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(`${state.nodes.length} nodes · ${edges.length} links`, 10, 18);

    requestAnimationFrame(drawNetwork);
  }
  drawNetwork();
}
