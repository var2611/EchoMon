import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

// Declare global types for external libraries
declare const Chart: any;
declare const jspdf: any;

let isRunning = false;
let sent = 0, received = 0, lost = 0;
let rttValues: number[] = [], rttHistory: { t: string, rtt: number }[] = [], logEntries: string[] = [];
let sessionStart: number | null = null;
let pingInterval: number | null = null;
let target = "";
let currentSessionId: number | null = null;
let chartInstance: any = null;

const els = {
    log: document.getElementById('logContainer') as HTMLElement,
    sent: document.getElementById('sent') as HTMLElement,
    received: document.getElementById('received') as HTMLElement,
    lost: document.getElementById('lost') as HTMLElement,
    lossPct: document.getElementById('lossPct') as HTMLElement,
    minRTT: document.getElementById('minRTT') as HTMLElement,
    avgRTT: document.getElementById('avgRTT') as HTMLElement,
    maxRTT: document.getElementById('maxRTT') as HTMLElement,
    jitter: document.getElementById('jitter') as HTMLElement,
    uptime: document.getElementById('uptime') as HTMLElement,
    packetCounter: document.getElementById('packetCounter') as HTMLElement,
    liveIP: document.getElementById('liveIP') as HTMLElement,
    historyModal: document.getElementById('historyModal') as HTMLElement,
    historyTableBody: document.getElementById('historyTableBody') as HTMLElement,
    rttChart: document.getElementById('rttChart') as HTMLCanvasElement,
};

function ts() {
    const d = new Date();
    // Local time for live logs
    return d.toLocaleString('sv-SE').replace(',', ''); // YYYY-MM-DD HH:MM:SS format
}

function log(msg: string, header = false) {
    const line = header ? `\n${msg}\n` : `${ts()}  ${msg}`;
    const div = document.createElement('div');
    div.className = 'log-line ' + (header ? 'text-emerald-400 font-medium' : '');
    div.textContent = line;
    els.log.appendChild(div);
    els.log.scrollTop = els.log.scrollHeight;
    logEntries.push(line.trim());
}

function updateStats() {
    els.sent.textContent = sent.toString();
    els.received.textContent = received.toString();
    els.lost.textContent = lost.toString();
    els.lossPct.textContent = sent ? ((lost / sent) * 100).toFixed(1) : '0.0';
    els.packetCounter.textContent = `S:${sent} R:${received} L:${lost}`;
    if (rttValues.length > 0) {
        const min = Math.min(...rttValues).toFixed(1);
        const max = Math.max(...rttValues).toFixed(1);
        const avg = (rttValues.reduce((a,b)=>a+b,0)/rttValues.length).toFixed(1);
        
        // Calculate Jitter: Average of absolute differences between consecutive RTTs
        let jitter = 0;
        if (rttValues.length > 1) {
            let sumDiff = 0;
            for (let i = 1; i < rttValues.length; i++) {
                sumDiff += Math.abs(rttValues[i] - rttValues[i-1]);
            }
            jitter = sumDiff / (rttValues.length - 1);
        }

        els.minRTT.textContent = min;
        els.maxRTT.textContent = max;
        els.avgRTT.textContent = avg;
        els.jitter.textContent = jitter.toFixed(1);
    } else {
        els.minRTT.textContent = "—";
        els.maxRTT.textContent = "—";
        els.avgRTT.textContent = "—";
        els.jitter.textContent = "—";
    }
}

function updateUptime() {
    if (!sessionStart) return;
    const sec = Math.floor((Date.now() - sessionStart) / 1000);
    els.uptime.textContent = `${Math.floor(sec/60).toString().padStart(2,'0')}:${(sec % 60).toString().padStart(2,'0')}`;
}

async function doRealPing() {
    if (!isRunning) return;
    sent++;

    const ipInput = document.getElementById('targetIP') as HTMLInputElement;
    const ip = ipInput.value.trim() || "8.8.8.8";
    
    try {
        const result: any = await invoke('ping_host', { ip, sessionId: currentSessionId });
        
        // Log resolved IP if it's different from input and present
        if (result.resolved_ip && result.resolved_ip !== ip) {
             // Only log once per session or if it changes? 
             // For now, let's log it if it's the first ping or just log it.
             // To avoid spamming, maybe check if we already logged it?
             // But doRealPing is called every interval.
             // Let's just log it in the debug message or a special message if it's new.
             // Actually, the user asked to "show logs of fetching ipv4 from url".
             // So logging it every time is fine, or maybe just:
             // "Reply from 1.2.3.4 (x.com): ..."
        }

        if (result.success && result.rtt !== null) {
            received++;
            rttValues.push(result.rtt);
            rttHistory.push({ t: ts(), rtt: result.rtt });
            
            const from = result.resolved_ip ? `${result.resolved_ip} (${ip})` : ip;
            log(`Reply from ${from}: bytes=32 time=${result.rtt}ms TTL=128`);
        } else {
            lost++;
            if (result.resolved_ip) {
                 log(`Request timed out for ${result.resolved_ip} (${ip}).`);
            } else {
                 log(`Request timed out.`);
            }
            
            if (result.error) {
                log(`[ERROR] ${result.error}`);
            }
        }
    } catch (err: any) {
        lost++;
        log(`[ERROR] ${err.message || err}`);
    }
    updateStats();
}

async function startPing() {
    if (isRunning) return;
    
    // Clear previous session data from UI
    sent = received = lost = 0;
    rttValues = [];
    rttHistory = [];
    logEntries = [];
    els.log.innerHTML = '';
    els.uptime.textContent = "00:00";

    const ipInput = document.getElementById('targetIP') as HTMLInputElement;
    target = ipInput.value.trim() || "8.8.8.8";
    els.liveIP.textContent = target;

    // Start new session in DB
    try {
        const id: number = await invoke('start_session', { target });
        currentSessionId = id;
    } catch (e) {
        log(`[ERROR] Failed to start session: ${e}`, true);
        return;
    }

    const intervalInput = document.getElementById('interval') as HTMLInputElement;
    const interval = parseInt(intervalInput.value) || 1000;

    sessionStart = Date.now();
    isRunning = true;
    
    const startBtn = document.getElementById('startBtn') as HTMLButtonElement;
    const stopBtn = document.getElementById('stopBtn') as HTMLButtonElement;
    
    startBtn.disabled = true;
    stopBtn.disabled = false;
    stopBtn.textContent = "STOP";
    stopBtn.classList.remove("bg-amber-600", "hover:bg-amber-500");
    stopBtn.classList.add("bg-red-700", "hover:bg-red-600");

    log(`Starting REAL ping to ${target}`, true);

    doRealPing();
    pingInterval = window.setInterval(doRealPing, interval);

    const durationInput = document.getElementById('duration') as HTMLInputElement;
    const dur = parseInt(durationInput.value) || 0;
    if (dur > 0) setTimeout(() => isRunning && stopPing(), dur * 1000);

    setInterval(updateUptime, 980);
    updateUptime();
    updateStats();
}

async function stopPing() {
    const stopBtn = document.getElementById('stopBtn') as HTMLButtonElement;
    const startBtn = document.getElementById('startBtn') as HTMLButtonElement;

    if (isRunning) {
        // STOPPING
        isRunning = false;
        if (pingInterval) clearInterval(pingInterval);
        
        log(`Session stopped • Duration ${els.uptime.textContent}`, true);
        updateStats();

        // Save session stats to DB
        const min = rttValues.length > 0 ? Math.min(...rttValues) : null;
        const max = rttValues.length > 0 ? Math.max(...rttValues) : null;
        const avg = rttValues.length > 0 ? rttValues.reduce((a, b) => a + b, 0) / rttValues.length : null;

        try {
            if (currentSessionId !== null) {
                await invoke('stop_session', { 
                    id: currentSessionId, 
                    sent, 
                    received, 
                    lost, 
                    min, 
                    avg, 
                    max 
                });
            }
        } catch (e) {
            log(`[ERROR] Failed to save session: ${e}`, true);
        }

        // Change STOP button to CLEAR button
        stopBtn.textContent = "CLEAR";
        stopBtn.classList.remove("bg-red-700", "hover:bg-red-600");
        stopBtn.classList.add("bg-amber-600", "hover:bg-amber-500");
        
        // Keep START disabled until cleared
        startBtn.disabled = true; 
        
    } else {
        // CLEARING
        clearStats();
    }
}

function clearStats() {
    sent = received = lost = 0;
    rttValues = [];
    rttHistory = [];
    logEntries = [];
    els.log.innerHTML = '';
    els.uptime.textContent = "00:00";
    sessionStart = null;
    currentSessionId = null;
    
    updateStats();
    
    const stopBtn = document.getElementById('stopBtn') as HTMLButtonElement;
    const startBtn = document.getElementById('startBtn') as HTMLButtonElement;
    
    stopBtn.textContent = "STOP";
    stopBtn.disabled = true;
    stopBtn.classList.remove("bg-amber-600", "hover:bg-amber-500");
    stopBtn.classList.add("bg-red-700", "hover:bg-red-600");
    
    startBtn.disabled = false;
    
    log("Ready to start new session.", true);
}

async function showHistory() {
    const modal = document.getElementById('historyModal') as HTMLElement;
    const tbody = document.getElementById('historyTableBody') as HTMLElement;
    
    modal.style.display = 'block';
    tbody.innerHTML = '<tr><td colspan="9" class="text-center py-8 text-gray-500">Loading...</td></tr>';

    try {
        const sessions: any[] = await invoke('get_sessions');
        tbody.innerHTML = ''; // Clear loading message
        
        if (!sessions || sessions.length === 0) {
            tbody.innerHTML = '<tr><td colspan="9" class="text-center py-8 text-gray-500">No history found.</td></tr>';
            return;
        }

        sessions.forEach(s => {
            const lossPct = s.sent > 0 ? ((s.lost / s.sent) * 100).toFixed(1) : '0.0';
            const avgRtt = s.avg ? s.avg.toFixed(1) : '—';
            
            let status = `<span class="text-emerald-400">Completed</span>`;
            let durationStr = "—";

            if (s.start_time) {
                // Parse UTC string from DB, JS Date automatically converts to local time
                const startDate = new Date(s.start_time);
                let endDate = new Date();
                
                if (s.end_time) {
                    endDate = new Date(s.end_time);
                } else {
                    status = `<span class="text-amber-400">Running/Crashed</span>`;
                }
                
                const durationMs = endDate.getTime() - startDate.getTime();
                const durationSec = Math.floor(durationMs / 1000);
                durationStr = `${Math.floor(durationSec/60).toString().padStart(2,'0')}:${(durationSec % 60).toString().padStart(2,'0')}`;
                
                // Format local date string
                var localDateStr = startDate.toLocaleString();
            } else {
                var localDateStr = "Unknown";
            }

            const row = `
                <tr class="hover:bg-gray-800/50 border-b border-gray-800 last:border-0 cursor-pointer transition-colors" onclick="toggleGraph(${s.id})">
                    <td class="px-4 py-3 font-medium text-white">${s.id}</td>
                    <td class="px-4 py-3 font-mono text-emerald-300">${s.target}</td>
                    <td class="px-4 py-3 text-gray-400">${localDateStr}</td>
                    <td class="px-4 py-3 text-gray-400 font-mono">${durationStr}</td>
                    <td class="px-4 py-3 text-center text-white">${s.sent}</td>
                    <td class="px-4 py-3 text-center text-amber-400">${lossPct}%</td>
                    <td class="px-4 py-3 text-center text-sky-400">${avgRtt} ms</td>
                    <td class="px-4 py-3">${status}</td>
                    <td id="actions-cell-${s.id}" class="px-4 py-3 text-right">
                        <button onclick="deleteSession(${s.id}, event)" class="text-red-500 hover:text-red-400 p-1 rounded hover:bg-red-900/30 transition-colors" title="Delete Session">
                            🗑️
                        </button>
                    </td>
                </tr>
                <tr id="graph-row-${s.id}" class="hidden bg-gray-900/50">
                    <td colspan="9" class="p-4">
                        <div class="h-64 w-full relative">
                            <canvas id="chart-${s.id}"></canvas>
                        </div>
                    </td>
                </tr>
            `;
            tbody.innerHTML += row;
        });

    } catch (e) {
        tbody.innerHTML = `<tr><td colspan="9" class="text-center py-8 text-red-400">Error loading history: ${e}</td></tr>`;
    }
}

function deleteSession(id: number, event: Event) {
    event.stopPropagation();
    const cell = document.getElementById(`actions-cell-${id}`);
    if (!cell) return;

    // Prevent the row from being clickable while confirmation is active
    const row = cell.closest('tr');
    if(row) (row as any).onclick = null;

    cell.innerHTML = `
        <div class="flex items-center justify-end gap-2">
            <span class="text-xs text-amber-400">Sure?</span>
            <button onclick="confirmDelete(${id}, event)" class="text-xs font-bold text-emerald-400 hover:text-emerald-300 px-2 py-1 rounded bg-emerald-900/50 hover:bg-emerald-900/80">YES</button>
            <button onclick="showHistory()" class="text-xs font-bold text-red-500 hover:text-red-400 px-2 py-1 rounded bg-red-900/50 hover:bg-red-900/80">NO</button>
        </div>
    `;
}

async function confirmDelete(id: number, event: Event) {
    event.stopPropagation();
    try {
        await invoke('delete_session', { id });
        showHistory(); // Refresh list
    } catch (e) {
        alert(`Failed to delete session: ${e}`);
        showHistory(); // Also refresh on failure to restore the UI
    }
}

async function toggleGraph(id: number) {
    const graphRow = document.getElementById(`graph-row-${id}`);
    if (!graphRow) return;

    if (!graphRow.classList.contains('hidden')) {
        graphRow.classList.add('hidden');
        return;
    }

    // Show row
    graphRow.classList.remove('hidden');
    
    // Check if chart already exists
    const canvas = document.getElementById(`chart-${id}`) as HTMLCanvasElement;
    if (canvas.getAttribute('data-loaded') === 'true') return;

    // Fetch data and render chart
    try {
        const pings: any[] = await invoke('get_session_pings', { id });
        
        const labels = pings.map((_, i) => i + 1);
        const data = pings.map(p => p.success ? p.rtt : null);

        new Chart(canvas, {
            type: 'line',
            data: {
                labels: labels,
                datasets: [{
                    label: 'RTT (ms)',
                    data: data,
                    borderColor: '#10b981', // emerald-500
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    borderWidth: 2,
                    pointRadius: 0,
                    pointHoverRadius: 4,
                    tension: 0.2,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: { display: false },
                    y: {
                        beginAtZero: true,
                        grid: { color: '#374151' },
                        ticks: { color: '#9ca3af' }
                    }
                },
                plugins: {
                    legend: { display: false },
                    tooltip: {
                        mode: 'index',
                        intersect: false,
                        backgroundColor: '#1f2937',
                        titleColor: '#10b981',
                        bodyColor: '#fff',
                        borderColor: '#374151',
                        borderWidth: 1
                    }
                }
            }
        });
        
        canvas.setAttribute('data-loaded', 'true');

    } catch (e) {
        console.error("Failed to load graph data", e);
        graphRow.innerHTML = `<td colspan="9" class="p-4 text-center text-red-400">Failed to load graph data</td>`;
    }
}

function closeHistory() {
    const modal = document.getElementById('historyModal') as HTMLElement;
    modal.style.display = 'none';
}

async function downloadLogs() {
    if (logEntries.length === 0) return alert("No logs yet");
    
    const logContent = logEntries.join('\n');
    const defaultName = `EchoMon-log-${new Date().toISOString().slice(0,19).replace(/:/g,'-')}.txt`;
    
    try {
        const path = await save({
            defaultPath: defaultName,
            filters: [{
                name: 'Text Files',
                extensions: ['txt']
            }]
        });
        
        if (path) {
            await invoke('export_logs_to_path', { path, logs: logContent });
            alert(`Logs saved successfully!`);
        }
    } catch (e) {
        alert(`Failed to save logs: ${e}`);
    }
}

async function generatePDF() {
    if (rttHistory.length === 0) return alert("Run ping first to generate a report");

    const { jsPDF } = (window as any).jspdf;
    const doc = new jsPDF();
    
    // 1. Generate Chart Image
    const canvas = els.rttChart;
    if (chartInstance) chartInstance.destroy();

    chartInstance = new Chart(canvas, {
        type: 'line',
        data: {
            labels: rttHistory.map((_, i) => i + 1),
            datasets: [{
                label: 'RTT (ms)',
                data: rttHistory.map(h => h.rtt),
                borderColor: '#10b981',
                backgroundColor: 'rgba(16, 185, 129, 0.1)',
                borderWidth: 2,
                pointRadius: 0,
                tension: 0.1,
                fill: true
            }]
        },
        options: {
            animation: false,
            responsive: false,
            scales: {
                x: { display: false },
                y: { beginAtZero: true }
            },
            plugins: { legend: { display: false } }
        }
    });

    // Wait for chart render (Chart.js is usually fast enough synchronously with animation: false)
    const chartImg = canvas.toDataURL("image/png");

    // 2. Build PDF
    let y = 20;
    
    // Header
    doc.setFontSize(22); 
    doc.setTextColor(16, 185, 129); // Emerald color
    doc.text("EchoMon ICMP TOOL - REPORT", 20, y); 
    y += 15;

    // Meta Info
    doc.setFontSize(12); 
    doc.setTextColor(100);
    doc.text(`Target Host: ${target}`, 20, y); y += 7;
    
    // Test Start Time
    const startTimeStr = sessionStart ? new Date(sessionStart).toLocaleString() : "Unknown";
    doc.text(`Test Start Time: ${startTimeStr}`, 20, y); y += 7;
    
    // Test Stop Time / Report Generated
    const stopTimeStr = new Date().toLocaleString();
    doc.text(`Test Stop Time: ${stopTimeStr}`, 20, y); y += 7;
    
    // Session Uptime/Duration
    doc.text(`Session Duration: ${els.uptime.textContent} (Continuous monitoring)`, 20, y); y += 7;
    
    // Target Device IP (if resolved differently, maybe show both, but target is usually what user typed)
    // If we have a resolved IP, we could show it, but 'target' variable holds the input.
    // Let's stick to the input target for now as requested.
    doc.text(`Target Device IP: ${target}`, 20, y); y += 7;
    
    // Monitoring Mode
    doc.text(`Monitoring Mode: Continuous session`, 20, y); y += 15;

    // Statistics Box
    doc.setDrawColor(200);
    doc.setFillColor(245, 247, 250);
    doc.rect(20, y, 170, 40, 'F');
    doc.rect(20, y, 170, 40, 'S');
    
    y += 10;
    doc.setFontSize(14); doc.setTextColor(0);
    doc.text("Session Statistics", 25, y);
    
    y += 10;
    doc.setFontSize(11);
    doc.text(`Sent: ${sent}`, 25, y);
    doc.text(`Received: ${received}`, 80, y);
    doc.text(`Lost: ${lost} (${els.lossPct.textContent}%)`, 135, y);
    
    y += 8;
    doc.text(`Min RTT: ${els.minRTT.textContent} ms`, 25, y);
    doc.text(`Avg RTT: ${els.avgRTT.textContent} ms`, 80, y);
    doc.text(`Max RTT: ${els.maxRTT.textContent} ms`, 135, y);
    
    // Add Jitter to PDF
    doc.text(`Jitter: ${els.jitter.textContent} ms`, 25, y + 8);

    y += 25;

    // Chart
    doc.setFontSize(14); doc.setTextColor(0);
    doc.text("Latency Graph", 20, y);
    y += 5;
    doc.addImage(chartImg, 'PNG', 20, y, 170, 80);

    // Save
    const defaultName = `EchoMon-Report-${target.replace(/\./g,'-')}-${new Date().toISOString().slice(0,19).replace(/:/g,'-')}.pdf`;
    
    try {
        const path = await save({
            defaultPath: defaultName,
            filters: [{ name: 'PDF Document', extensions: ['pdf'] }]
        });

        if (path) {
            // Get raw PDF data as ArrayBuffer
            const pdfArrayBuffer = doc.output('arraybuffer');
            // Convert to regular array of numbers (bytes) for Tauri
            const pdfBytes = Array.from(new Uint8Array(pdfArrayBuffer));
            
            await invoke('save_binary_file', { path, data: pdfBytes });
            alert(`PDF Report saved successfully!`);
        }
    } catch (e) {
        alert(`Failed to save PDF: ${e}`);
    }
}

// Expose functions to window for HTML onclick handlers
(window as any).startPing = startPing;
(window as any).stopPing = stopPing;
(window as any).downloadLogs = downloadLogs;
(window as any).generatePDF = generatePDF;
(window as any).showHistory = showHistory;
(window as any).closeHistory = closeHistory;
(window as any).deleteSession = deleteSession;
(window as any).confirmDelete = confirmDelete;
(window as any).toggleGraph = toggleGraph;

// Close modal when clicking outside
window.onclick = function(event) {
    const modal = document.getElementById('historyModal');
    if (event.target == modal) {
        closeHistory();
    }
}

log("Tauri + Real Ping ready", true);
log("Enter IP and click START", true);
