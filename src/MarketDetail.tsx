import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import type { MarketItem, McpDetail } from './types';

export function MarketDetail({ item, busy, error, onClose, onPreview, onMcp }: {
  item: MarketItem; busy: boolean; error?: string; onClose: () => void;
  onPreview: (command: string, args: Record<string, unknown>) => void;
  onMcp: (content: string) => void;
}) {
  const [name, setName] = useState('');
  const [revision, setRevision] = useState('main');
  const [subdirectory, setSubdirectory] = useState('');
  const github = item.source === 'GitHub';
  const plugin = item.source === 'Claude Marketplace';
  const [detail, setDetail] = useState<McpDetail | null>(null);
  const [detailError, setDetailError] = useState('');
  const [reviewed, setReviewed] = useState(false);
  useEffect(() => {
    if (item.source !== 'MCP Registry') return;
    let current = true;
    void invoke<McpDetail>('mcp_detail', { name: item.name, version: item.version ?? 'latest' }).then(value => {
      if (current) setDetail(value);
    }).catch(reason => { if (current) setDetailError(String(reason)); });
    return () => { current = false; };
  }, [item.source, item.name, item.version]);
  return <div className="overlay"><section className="modal" role="dialog" aria-modal="true" aria-label="市场组件详情">
    <button className="close" disabled={busy} aria-label="关闭市场详情" onClick={onClose}><X /></button>
    <span className="eyebrow">{item.source}</span><h2>{item.name}</h2><p>{item.description}</p>
    {error && <p role="alert" className="banner error">{error}</p>}
    <p>{item.stars === null ? '来源未提供热度数据' : `GitHub 仓库 Star：${item.stars.toLocaleString()}`} · 获取于 {new Date(item.fetchedAt * 1000).toLocaleString()}</p>
    {github ? <><p>选择仓库中的 Skill 目录。生成预览时解析为固定提交，并读取完整包；不会运行仓库脚本。</p>
      <label>安装目录名<input value={name} onChange={e => setName(e.target.value)} /></label>
      <label>分支、标签或提交<input value={revision} onChange={e => setRevision(e.target.value)} /></label>
      <label>仓库内 Skill 目录（留空表示根目录）<input value={subdirectory} onChange={e => setSubdirectory(e.target.value)} placeholder="skills/example" /></label>
      <button className="primary" disabled={busy || !name.trim() || !revision.trim()} onClick={() => onPreview('preview_github', { repository: item.name, revision, subdirectory, name, action: 'install', id: null })}>解析来源并预览安装</button>
    </> : plugin ? <><p>此版本提供所属 Agent 的手动管理命令，不会执行命令或修改插件缓存。</p><label>插件标识（plugin@marketplace）<input value={name} onChange={e => setName(e.target.value)} /></label><button disabled={busy || !name.includes('@')} onClick={() => onPreview('preview_plugin', { action: 'install', name, scope: 'user' })}>查看手动安装说明</button></> : <>
      {detailError ? <p role="alert" className="banner error">无法读取注册表详情：{detailError}</p> : !detail ? <p role="status">正在读取服务器版本与传输信息…</p> : <>
        <p>版本 {detail.version} · 获取于 {new Date(detail.fetchedAt * 1000).toLocaleString()}</p>
        <h3>远程传输与请求头要求</h3><pre>{JSON.stringify(detail.metadata.remotes ?? [], null, 2)}</pre>
        <h3>运行时包与环境变量要求</h3><pre>{JSON.stringify(detail.metadata.packages ?? [], null, 2)}</pre>
        <p className="banner">以上为来源发布的传输、请求头和环境变量要求。必填凭据需自行补充；生成注册配置不表示服务器已安装或可运行。</p>
        {detail.config ? <><p>可预填 Streamable HTTP 地址，随后选择 Agent、注册名称及范围，审阅并补充所需凭据。</p><label className="confirm"><input type="checkbox" checked={reviewed} onChange={e => setReviewed(e.target.checked)} />我已审阅传输地址和凭据要求</label><button disabled={busy || !reviewed} onClick={() => onMcp(JSON.stringify(detail.config, null, 2))}>使用此地址编辑注册配置</button></> : <><p>没有可预填的 Streamable HTTP 地址。包类型或其他传输需要显式配置，本应用不会安装运行时包。</p><button disabled={busy} onClick={() => onMcp('{}')}>手动填写 MCP 注册配置</button></>}
      </>}
    </>}
  </section></div>;
}
