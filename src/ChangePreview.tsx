import { useRef, useState } from 'react';
import { X } from 'lucide-react';
import type { Plan } from './types';

export function ChangePreview({ plan, enabled, busy, error, onClose, onApply }: {
  plan: Plan; enabled: boolean; busy: boolean; error?: string;
  onClose: () => void; onApply: (id: string) => Promise<void>;
}) {
  const [confirmed, setConfirmed] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const locked = useRef(false);
  async function apply() {
    if (!enabled || !confirmed || busy || locked.current) return;
    locked.current = true;
    setSubmitting(true);
    try { await onApply(plan.id); }
    finally { locked.current = false; setSubmitting(false); setConfirmed(false); }
  }
  return <div className="overlay"><section className="modal wide" role="dialog" aria-modal="true" aria-label="变更预览">
    <button className="close" aria-label="关闭预览" disabled={busy || submitting} onClick={onClose}><X /></button>
    <span className="eyebrow">CHANGE PREVIEW</span><h2>{plan.title}</h2>
    {error && <p role="alert" className="banner error">{error}</p>}
    {plan.warnings.map((w, i) => <p className="warning" key={i}>{w}</p>)}
    {plan.changes.map(c => <details open key={c.path}>
      <summary>{c.after === null ? '删除' : c.before === null ? '创建' : '修改'} · {c.path}</summary>
      <div className="diff"><pre>{c.before ?? '（不存在）'}</pre><pre>{c.after ?? '（移除）'}</pre></div>
    </details>)}
    {plan.changes.length === 0 ? <p className="banner">此预览没有可应用的文件变更。手动命令需在所属 Agent 中执行，完成后返回扫描。</p> : <>
      {!enabled && <p className="banner">当前版本尚未开放原生写入，目前仅可预览。</p>}
      <label className="confirm"><input type="checkbox" checked={confirmed} disabled={submitting || busy} onChange={e => setConfirmed(e.target.checked)} />我已审阅以上文件变更，同意备份后应用。</label>
      <button className="primary" disabled={!enabled || !confirmed || busy || submitting} onClick={() => void apply()}>{submitting ? '正在应用…' : '备份并应用'}</button>
    </>}
  </section></div>;
}
