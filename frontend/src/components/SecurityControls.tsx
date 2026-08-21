import { ShieldCheck, SlidersHorizontal } from "lucide-react";

import { useAgentGuardStore } from "../store";

const controls = [
  { key: "autoApproveAstSafe", label: "Auto-approve AST safe", detail: "Pass syntax-valid changes automatically" },
  { key: "requireHumanForDeletions", label: "Require human for deletions", detail: "Pause destructive filesystem operations" },
  { key: "strictSecretScanning", label: "Strict secret scanning", detail: "Block high-entropy credential patterns" },
] as const;

export function SecurityControls() {
  const values = useAgentGuardStore((state) => state.controls);
  const setControl = useAgentGuardStore((state) => state.setControl);
  return <section className="panel controls-panel"><div className="panel-header"><div><span className="eyebrow">Policy engine</span><h2><SlidersHorizontal size={17} /> Security controls</h2></div><ShieldCheck className="control-shield" size={21} /></div><div className="control-list">{controls.map((control) => <label className="control-row" key={control.key}><span><strong>{control.label}</strong><small>{control.detail}</small></span><input type="checkbox" checked={values[control.key]} onChange={(event) => setControl(control.key, event.target.checked)} /><i className="toggle" /></label>)}</div></section>;
}
