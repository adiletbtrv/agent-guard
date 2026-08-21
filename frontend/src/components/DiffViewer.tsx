import { ArrowRight, FileCode2 } from "lucide-react";

interface DiffViewerProps { before: string; after: string; fileName: string }

export function DiffViewer({ before, after, fileName }: DiffViewerProps) {
  return <section className="panel diff-panel"><div className="panel-header"><div><span className="eyebrow">Proposed change</span><h2><FileCode2 size={17} /> {fileName}</h2></div><span className="diff-label"><ArrowRight size={13} /> shadow preview</span></div><div className="diff-grid"><pre><code>{before.split("\n").map((line, index) => <span key={`${line}-${index}`} className="diff-line">{String(index + 1).padStart(2, "0")}  {line}</span>)}</code></pre><pre><code>{after.split("\n").map((line, index) => <span key={`${line}-${index}`} className={`diff-line ${line !== before.split("\n")[index] ? "line-added" : ""}`}>{String(index + 1).padStart(2, "0")}  {line}</span>)}</code></pre></div></section>;
}
