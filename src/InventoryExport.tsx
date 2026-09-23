import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Download, X } from 'lucide-react';

export function InventoryExport({ desktop, scannedAt, home, busy }: {
  desktop: boolean; scannedAt: number; home: string; busy: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [destination, setDestination] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState('');
  const lock = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (open) dialog.current?.showModal(); }, [open]);
  function close() { if (!lock.current) setOpen(false); }
  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!desktop || !scannedAt || lock.current) return;
    const path = destination.trim();
    if (!/^[a-z]:[\\/]/i.test(path) || !/\.json$/i.test(path)) {
      setError('请输入本机磁盘上的绝对路径，并使用 .json 扩展名。'); return;
    }
    lock.current = true; setPending(true); setError(''); setSaved('');
    try { setSaved(await invoke<string>('export_inventory', { destination: path })); }
    catch (reason) { setError(String(reason)); }
    finally { lock.current = false; setPending(false); }
  }
  return <>
    <button disabled={!desktop || !scannedAt || busy} title={!desktop ? '请在桌面客户端扫描后导出' : !scannedAt ? '请先扫描本机' : undefined} onClick={() => {
      setDestination(`${home.replace(/[\\/]+$/, '')}/agent-inventory-${scannedAt}.json`);
      setError(''); setSaved(''); setOpen(true);
    }}><Download size={15}/>导出 JSON</button>
    {open && <dialog ref={dialog} className="modal export-dialog" aria-labelledby="export-title" onCancel={event => { event.preventDefault(); close(); }}>
      <button className="close" aria-label="关闭导出" disabled={pending} onClick={close}><X/></button>
      <h2 id="export-title">导出本机组件清单</h2>
      <p>导出最近一次扫描的清单，不包含 MCP 配置凭据。目录必须已存在；不会覆盖已有文件。清单包含本机路径，分享前请检查。</p>
      <form onSubmit={event => void save(event)}>
        <label>JSON 保存路径<input autoFocus required value={destination} disabled={pending} onChange={event => { setDestination(event.target.value); setError(''); setSaved(''); }} placeholder="D:\\exports\\agent-inventory.json"/></label>
        {error && <p role="alert" className="banner error">{error}</p>}
        {saved && <p role="status" className="banner">已导出：{saved}</p>}
        <button className="primary" disabled={pending || !destination.trim() || !!saved}>{pending ? '正在导出…' : '保存 JSON'}</button>
      </form>
    </dialog>}
  </>;
}
