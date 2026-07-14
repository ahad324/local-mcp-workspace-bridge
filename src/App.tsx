import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';

interface Config {
  general: { workspace_folder: string; server_port: number; auto_start_server: boolean; auto_start_ngrok: boolean };
  ngrok: { executable_path: string; auth_token: string; region: string; tunnel_name: string };
  mcp: { server_port: number; server_name: string; server_description: string };
  app: { start_on_boot: boolean; theme: string; log_level: string };
}

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [ngrokUrl, setNgrokUrl] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [activeTab, setActiveTab] = useState('general');

  useEffect(() => {
    invoke<Config>('get_config').then(setConfig).catch(console.error);
    
    const interval = setInterval(() => {
      invoke<string | null>('get_ngrok_url').then(url => {
        if (url) setNgrokUrl(url);
      }).catch(() => {});
    }, 2000);
    
    return () => clearInterval(interval);
  }, []);

  const saveConfig = async () => {
    if (config) {
      await invoke('save_config_cmd', { config });
      alert('Settings saved successfully!');
    }
  };

  const startServer = async () => {
    await invoke('start_server');
    setIsRunning(true);
  };

  const stopServer = async () => {
    await invoke('stop_server');
    setIsRunning(false);
    setNgrokUrl(null);
  };

  if (!config) return <div className="loading">Loading configuration...</div>;

  return (
    <div className="container">
      <header>
        <h1>Local MCP Workspace Bridge</h1>
        <div className="status-bar">
          <span className={`status ${isRunning ? 'running' : 'stopped'}`}>
            {isRunning ? '● Running' : '○ Stopped'}
          </span>
          {ngrokUrl && (
            <span className="url">
              Public URL: <a href={ngrokUrl} target="_blank" rel="noopener noreferrer">{ngrokUrl}</a>
            </span>
          )}
          <button onClick={isRunning ? stopServer : startServer} className="toggle-btn">
            {isRunning ? 'Stop Server' : 'Start Server'}
          </button>
        </div>
      </header>

      <nav className="tabs">
        {['general', 'ngrok', 'mcp', 'app'].map(tab => (
          <button 
            key={tab} 
            className={activeTab === tab ? 'active' : ''} 
            onClick={() => setActiveTab(tab)}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </nav>

      <main className="tab-content">
        {activeTab === 'general' && (
          <section>
            <label>Workspace Folder:</label>
            <input type="text" value={config.general.workspace_folder} onChange={e => setConfig({...config, general: {...config.general, workspace_folder: e.target.value}})} placeholder="/path/to/workspace" />
            
            <label>Server Port:</label>
            <input type="number" value={config.general.server_port} onChange={e => setConfig({...config, general: {...config.general, server_port: parseInt(e.target.value) || 3000}})} />
            
            <label className="checkbox">
              <input type="checkbox" checked={config.general.auto_start_server} onChange={e => setConfig({...config, general: {...config.general, auto_start_server: e.target.checked}})} /> 
              Auto Start Server
            </label>
            
            <label className="checkbox">
              <input type="checkbox" checked={config.general.auto_start_ngrok} onChange={e => setConfig({...config, general: {...config.general, auto_start_ngrok: e.target.checked}})} /> 
              Auto Start ngrok
            </label>
          </section>
        )}

        {activeTab === 'ngrok' && (
          <section>
            <label>ngrok Executable Path:</label>
            <input type="text" value={config.ngrok.executable_path} onChange={e => setConfig({...config, ngrok: {...config.ngrok, executable_path: e.target.value}})} placeholder="ngrok" />
            
            <label>Auth Token:</label>
            <input type="password" value={config.ngrok.auth_token} onChange={e => setConfig({...config, ngrok: {...config.ngrok, auth_token: e.target.value}})} placeholder="Your ngrok auth token" />
            
            <label>Region (optional):</label>
            <input type="text" value={config.ngrok.region} onChange={e => setConfig({...config, ngrok: {...config.ngrok, region: e.target.value}})} placeholder="us, eu, ap, etc." />
            
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
              <option value="dark">Dark</option>
              <option value="light">Light</option>
            </select>
            
            <label>Log Level:</label>
            <select value={config.app.log_level} onChange={e => setConfig({...config, app: {...config.app, log_level: e.target.value}})}>
              <option value="error">Error</option>
              <option value="warn">Warn</option>
              <option value="info">Info</option>
              <option value="debug">Debug</option>
            </select>
          </section>
        )}
      </main>

      <footer>
        <button className="save-btn" onClick={saveConfig}>Save Settings</button>
      </footer>
    </div>
  );
}

export default App;