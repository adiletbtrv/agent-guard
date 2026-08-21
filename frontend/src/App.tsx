import { useEffect } from "react";
import { Activity, ArrowUpRight, CheckCircle2, Cpu, Github, Shield, Wifi, WifiOff } from "lucide-react";

import { DiffViewer } from "./components/DiffViewer";
import { EventStream } from "./components/EventStream";
import { SecurityControls } from "./components/SecurityControls";
import { connectToAgentGuard, useAgentGuardStore } from "./store";

const before = `const config = {\n  retries: 2,\n  timeout: 5000,\n};`;
const after = `const config = {\n  retries: 3,\n  timeout: 3500,\n  backoff: "exponential",\n};`;

export default function App() {
  const connected = useAgentGuardStore((state) => state.connected);
  const events = useAgentGuardStore((state) => state.events);
  useEffect(() => { const socket = connectToAgentGuard(); return () => socket.close(); }, []);
  const passed = events.filter((event) => event.status === "passed").length;
  const blocked = events.filter((event) => event.status === "blocked").length;
  return <div className="app-shell"><header className="topbar"><a className="brand" href="/"><span className="brand-mark"><Shield size={17} /></span><span>AgentGuard</span><b>CONTROL PLANE</b></a><div className="top-actions"><span className={`connection ${connected ? "is-online" : ""}`}><i />{connected ? "Daemon connected" : "Daemon offline"}</span><a className="icon-link" href="https://github.com" target="_blank" rel="noreferrer" aria-label="Open GitHub"><Github size={16} /></a><a className="profile" href="#system">AE</a></div></header><main className="main-content"><section className="hero-bar"><div><p className="kicker"><span className="pulse-dot" /> Local agent firewall / v0.1.0</p><h1>Keep autonomous code<br /><em>inside the lines.</em></h1><p className="hero-copy">AgentGuard validates, scans, and executes every agent action inside a reversible shadow workspace before it touches your repository.</p></div><div className="hero-meta"><div><span>Workspace</span><strong><Cpu size={14} /> local repository</strong></div><div><span>Protection</span><strong><CheckCircle2 className="green" size={14} /> active</strong></div></div></section><section className="metric-grid"><div className="metric"><span><Activity size={14} /> intercepted events</span><strong>{events.length.toString().padStart(2, "0")}</strong><small>since daemon start</small></div><div className="metric"><span><CheckCircle2 size={14} /> passed safely</span><strong className="green">{passed.toString().padStart(2, "0")}</strong><small>validated actions</small></div><div className="metric"><span><Shield size={14} /> prevented changes</span><strong className="amber">{blocked.toString().padStart(2, "0")}</strong><small>requiring review</small></div><div className="metric"><span><Wifi size={14} /> shadow sync</span><strong className="blue">100%</strong><small>rollback ready</small></div></section><div className="dashboard-grid"><EventStream /><SecurityControls /></div><DiffViewer before={before} after={after} fileName="src/config.ts" /></main><footer><span>AGENTGUARD / LOCAL-FIRST SECURITY</span><span>Every command gets a second chance <ArrowUpRight size={13} /></span><span><WifiOff size={12} /> MITM proxy ready</span></footer></div>;
}
