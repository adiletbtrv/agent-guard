import { create } from "zustand";

export type EventKind = "ast" | "secret_scan" | "sandbox";
export type EventStatus = "passed" | "blocked" | "running" | "error";

export interface AgentEvent {
  id: string;
  timestamp: string;
  kind: EventKind;
  status: EventStatus;
  message: string;
  command?: string | null;
  path?: string | null;
}

interface SecurityControls {
  autoApproveAstSafe: boolean;
  requireHumanForDeletions: boolean;
  strictSecretScanning: boolean;
}

interface AgentGuardStore {
  events: AgentEvent[];
  connected: boolean;
  controls: SecurityControls;
  setConnected: (connected: boolean) => void;
  addEvent: (event: AgentEvent) => void;
  setControl: <K extends keyof SecurityControls>(key: K, value: SecurityControls[K]) => void;
  clearEvents: () => void;
}

export const useAgentGuardStore = create<AgentGuardStore>((set) => ({
  events: [],
  connected: false,
  controls: {
    autoApproveAstSafe: true,
    requireHumanForDeletions: true,
    strictSecretScanning: true,
  },
  setConnected: (connected) => set({ connected }),
  addEvent: (event) => set((state) => ({ events: [event, ...state.events].slice(0, 250) })),
  setControl: (key, value) => set((state) => ({ controls: { ...state.controls, [key]: value } })),
  clearEvents: () => set({ events: [] }),
}));

export function connectToAgentGuard() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const socket = new WebSocket(`${protocol}//${window.location.host}/ws`);
  const store = useAgentGuardStore.getState();
  socket.addEventListener("open", () => store.setConnected(true));
  socket.addEventListener("message", (message) => {
    try { store.addEvent(JSON.parse(message.data as string) as AgentEvent); } catch { /* Ignore malformed external messages. */ }
  });
  socket.addEventListener("close", () => store.setConnected(false));
  socket.addEventListener("error", () => store.setConnected(false));
  return socket;
}
