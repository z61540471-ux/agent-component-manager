import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Operation } from './types';

interface RecoveryRecord {
  id: string; title: string; status: string; message: string;
  entries: { path: string; before_hash: string | null; after_hash: string | null; attempted: boolean }[];
}

export function RecoveryPanel({ enabled, onRecovered }: {
  enabled: boolean; onRecovered: () => Promise<void>;
}) {
  const [records, setRecords] = useState<RecoveryRecord[]>([]);
  const [confirmed, setConfirmed] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const locked = useRef(false);
  useEffect(() => {
    let current = true;
    void invoke<RecoveryRecord[]>('recovery').then(value => {
      if (current) setRecords(value);
    }).catch(reason => { if (current) setError(String(reason)); });
    return () => { current = false; };
  }, []);
  async function recover(id: string) {
    if (!enabled || confirmed !== id || locked.current) return;
    locked.current = true;
    setBusy(true); setError(''); setMessage('');
    try {
      const result = await invoke<Operation>('recover_operation', { id, confirmed: true });
      setMessage(`${result.status}: ${result.message}`);
      setRecords(await invoke<RecoveryRecord[]>('recovery'));
      await onRecovered();
    } catch (reason) { setError(String(reason)); }
    finally { locked.current = false; setBusy(false); setConfirmed(''); }
  }
  const pending = records.filter(record => !['completed', 'rolled_back'].includes(record.status));
  if (!pending.length && !error && !message) return null;
  return <section className="panel settings" aria-label="中断操作恢复">
    <h2>中断操作恢复</h2>
    <p>恢复到操作前的文件内容。若当前文件与操作记录不符，会保留外部修改并报告冲突。恢复不运行安装脚本。</p>
    {error && <p role="alert" className="banner error">{error}</p>}
    {message && <p role="status" className="banner">{message}</p>}
    {!enabled && <p className="banner">恢复写入尚未开放，目前可查看受影响文件。</p>}
    {pending.map(record => <details key={record.id} open>
      <summary>{record.title} · {record.status}</summary><p>{record.message}</p>
      <ul>{record.entries.filter(entry => entry.attempted).map(entry => <li key={entry.path}>{entry.before_hash === null ? '移除本次创建的文件' : '从备份恢复原内容'}：{entry.path}</li>)}</ul>
      <label className="confirm"><input type="checkbox" checked={confirmed === record.id} disabled={busy || !enabled} onChange={event => setConfirmed(event.target.checked ? record.id : '')} />我已审阅受影响文件，确认恢复到操作前状态。</label>
      <button disabled={!enabled || busy || confirmed !== record.id} onClick={() => void recover(record.id)}>校验备份并恢复</button>
    </details>)}
  </section>;
}
