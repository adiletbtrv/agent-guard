import { useMemo } from "react";
import { CheckCircle2, CircleAlert, FileCode2, LockKeyhole, Play, ShieldAlert, Terminal } from "lucide-react";

import { useAgentGuardStore, type AgentEvent } from "../store";

const icons = { ast: FileCode2, secret_scan: LockKeyhole, sandbox: Terminal } as const;

function EventRow({ event }: { event: AgentEvent }) {
  const Icon = icons[event.kind];
  const StatusIcon = event.status === "passed" ? CheckCircle2 : event.status === "blocked" || event.status === "error" ? ShieldAlert : Play;
  const time = new Date(event.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  return (
    <li className="event-row">
      <div className="event-icon"><Icon size={15} /></div>
      <div className="event-body"><div className="event-meta"><span>{event.kind.replace("_", " ")}</span><time>{time}</time></div><p>{event.message}</p>{event.path ? <small>{event.path}</small> : null}</div>
      <StatusIcon className={`status-${event.status}`} size={16} />
    </li>
  );
}

export function EventStream() {
  const events = useAgentGuardStore((state) => state.events);
  const clearEvents = useAgentGuardStore((state) => state.clearEvents);
  const counts = useMemo(() => events.reduce((result, event) => { result[event.status] = (result[event.status] ?? 0) + 1; return result; }, {} as Record<string, number>), [events]);
  return (
    <section className="panel event-panel">
      <div className="panel-header"><div><span className="eyebrow">Live telemetry</span><h2>Event stream</h2></div><div className="event-actions"><span className="event-count">{counts.blocked ?? 0} blocked</span><button type="button" onClick={clearEvents}>Clear</button></div></div>
      <div className="stream-window">{events.length ? <ul>{events.map((event) => <EventRow key={event.id} event={event} />)}</ul> : <div className="empty-stream"><CircleAlert size={18} /><p>Waiting for intercepted agent actions</p><span>AST checks, secret scans, and sandbox runs appear here in real time.</span></div>}</div>
    </section>
  );
}
