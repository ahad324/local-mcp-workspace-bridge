import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';

interface Config {
  general: { workspace_folder: string; server_port: number; auto_start_server: boolean; auto_start_ngrok: boolean };
  ngrok: { executable_path: string; auth_token: string; region: string; tunnel_name: string };
  mcp: { server_port: number; server_name: string; server_description: string };
  app: { start_on_boot: boolean; theme: string; log_level: string };
}

interface LogEntry { time: number; level: string; message: string; }

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [ngrokUrl, setNgrokUrl] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [activeTab, setActiveTab] = useState('general');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<Config>('get_config').then(setConfig).catch(console.error);
    
    // Poll for ngrok URL
    const urlInterval = setInterval(() => {
      invoke<string | null>('get_ngrok_url').then(url => { if (url) setNgrokUrl(url); }).catch(() => {});
    }, 2000);
    
    // NEW: Poll for logs every 500ms
    const logInterval = setInterval(() => {
      invoke<LogEntry[]>('get_logs').then(newLogs => {
        if (newLogs.length > 0) {
          setLogs(prev => [...prev, ...newLogs].slice(-200));
        }
      }).catch(() => {});
    }, 500);

    return () => { clearInterval(urlInterval); clearInterval(logInterval); };
  }, []);

  useEffect(() => { logsEndRef.current?.scrollIntoView({ behavior: 'smooth' }); }, [logs]);

  const saveConfig = async () => { if (config) { await invoke('save_config_cmd', { config }); alert('Settings saved!'); } };
  const startServer = async () => { await invoke('start_server'); setIsRunning(true); };
  const stopServer = async () => { await invoke('stop_server'); setIsRunning(false); setNgrokUrl(null); };
  const clearLogs = () => setLogs([]);

  if (!config) return <div className="loading">Loading configuration...</div>;

  const formatTime = (ms: number) => new Date(ms).toLocaleTimeString();

  return (
    <div className="container">
      <header>
        <h1>Local MCP Workspace Bridge</h1>
        <div className="status-bar">
          <span className={`status ${isRunning ? 'running' : 'stopped'}`}>{isRunning ? '● Running' : '○ Stopped'}</span>
          {ngrokUrl && <span className="url">Public URL: <a href={ngrokUrl} target="_blank" rel="noopener noreferrer">{ngrokUrl}</a></span>}
          <button onClick={isRunning ? stopServer : startServer} className="toggle-btn">{isRunning ? 'Stop Server' : 'Start Server'}</button>
        </div>
      </header>

      <nav className="tabs">
        {['general', 'ngrok', 'mcp', 'app', 'logs'].map(tab => (
          <button key={tab} className={activeTab === tab ? 'active' : ''} onClick={() => setActiveTab(tab)}>
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </nav>

      <main className="tab-content">
        {activeTab === 'general' && (
          <section>
            <label>Workspace Folder:</label>
            <input type="text" value={config.general.workspace_folder} onChange={e => setConfig({...config, general: {...config.general, workspace_folder: e.target.value}})} />
            <label>Server Port:</label>
            <input type="number" value={config.general.server_port} onChange={e => setConfig({...config, general: {...config.general, server_port: parseInt(e.target.value) || 3000}})} />
            <label className="checkbox"><input type="checkbox" checked={config.general.auto_start_server} onChange={e => setConfig({...config, general: {...config.general, auto_start_server: e.target.checked}})} /> Auto Start Server</label>
            <label className="checkbox"><input type="checkbox" checked={config.general.auto_start_ngrok} onChange={e => setConfig({...config, general: {...config.general, auto_start_ngrok: e.target.checked}})} /> Auto Start ngrok</label>
          </section>
        )}
        {activeTab === 'ngrok' && (
          <section>
            <label>ngrok Executable Path:</label>
            <input type="text" value={config.ngrok.executable_path} onChange={e => setConfig({...config, ngrok: {...config.ngrok, executable_path: e.target.value}})} />
            <label>Auth Token:</label>
            <input type="password" value={config.ngrok.auth_token} onChange={e => setConfig({...config, ngrok: {...config.ngrok, auth_token: e.target.value}})} />
            <label>Region (optional):</label>
            <input type="text" value={config.ngrok.region} onChange={e => setConfig({...config, ngrok: {...config.ngrok, region: e.target.value}})} />
            <label>Tunnel Name (optional):</label>
            <input type="text" value={config.ngrok.tunnel_name} onChange={e => setConfig({...config, ngrok: {...config.ngrok, tunnel_name: e.target.value}})} />
          </section>
        )}
        {activeTab === 'mcp' && (
          <section>
            <label>Server Port:</label>
            <input type="number" value={config.mcp.server_port} onChange={e => setConfig({...config, mcp: {...config.mcp, server_port: parseInt(e.target.value) || 3001}})} />
            <label>Server Name:</label>
            <input type="text" value={config.mcp.server_name} onChange={e => setConfig({...config, mcp: {...config.mcp, server_name: e.target.value}})} />
            <label>Server Description:</label>
            <input type="text" value={config.mcp.server_description} onChange={e => setConfig({...config, mcp: {...config.mcp, server_description: e.target.value}})} />
          </section>
        )}
        {activeTab === 'app' && (
          <section>
            <label>Theme:</label>
            <select value={config.app.theme} onChange={e => setConfig({...config, app: {...config.app, theme: e.target.value}})}>
              <option value="dark">Dark</option><option value="light">Light</option>
            </select>
            <label>Log Level:</label>
            <select value={config.app.log_level} onChange={e => setConfig({...config, app: {...config.app, log_level: e.target.value}})}>
              <option value="error">Error</option><option value="warn">Warn</option><option value="info">Info</option><option value="debug">Debug</option>
            </select>
          </section>
        )}
        {activeTab === 'logs' && (
          <section className="logs-section">
            <div className="logs-header">
              <h3>Real-Time Server Logs</h3>
              <button onClick={clearLogs} className="clear-btn">Clear</button>
            </div>
            <div className="logs-console">
              {logs.length === 0 && <div className="empty-logs">No logs yet. Start the server and trigger an action.</div>}
              {logs.map((log, idx) => (
                <div key={idx} className={`log-entry log-${log.level.toLowerCase()}`}>
                  <span className="log-time">[{formatTime(log.time)}]</span>
                  <span className="log-level">{log.level}</span>
                  <span className="log-msg">{log.message}</span>
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          </section>
        )}
      </main>
      {activeTab !== 'logs' && <footer><button className="save-btn" onClick={saveConfig}>Save Settings</button></footer>}
    </div>
  );
}

export default App;