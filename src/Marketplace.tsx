import { useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowUpRight, Search, Store } from 'lucide-react';
import type { MarketItem, MarketResult } from './types';

export function Marketplace({ desktop, busy, onDetail }: { desktop: boolean; busy: boolean; onDetail: (item: MarketItem) => void }) {
  const [source, setSource] = useState('GitHub');
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<MarketResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const active = useRef(false);
  const identity = useRef(0);
  const cursors = useRef(new Set<string>());
  const pages = useRef(0);
  function reset() { identity.current++; setResult(null); setError(''); cursors.current.clear(); pages.current = 0; }
  async function search(more = false) {
    if (active.current || busy) return;
    if (!desktop) { setError('请在桌面客户端中搜索市场。'); return; }
    const cursor = more ? result?.nextCursor ?? null : null;
    if (more && (!cursor || cursors.current.has(cursor) || pages.current >= 34)) return;
    const request = identity.current;
    active.current = true; setLoading(true); setError('');
    if (!more) { setResult(null); cursors.current.clear(); pages.current = 0; }
    try {
      const page = await invoke<MarketResult>('market', { source, query: query.trim(), cursor });
      if (identity.current !== request) return;
      if (cursor) cursors.current.add(cursor);
      pages.current++;
      if (page.nextCursor && cursors.current.has(page.nextCursor)) {
        page.nextCursor = null; page.message = '来源返回了重复的翻页标记，已停止加载。';
      }
      setResult(previous => ({ ...page,
        stale: page.stale || (more && !!previous?.stale),
        message: page.message || (more ? previous?.message ?? null : null),
        items: [...new Map([...(more ? previous?.items ?? [] : []), ...page.items].map(item => [JSON.stringify([item.source, item.name, item.version]), item])).values()],
      }));
    } catch (e) { if (identity.current === request) setError(String(e)); }
    finally { active.current = false; setLoading(false); }
  }
  return <>
    <form className="filters panel" onSubmit={e => { e.preventDefault(); void search(); }}>
      <select aria-label="市场来源" value={source} onChange={e => { reset(); setSource(e.target.value); }}><option>GitHub</option><option>MCP Registry</option><option>Claude Marketplace</option></select>
      <label className="search"><Search size={18}/><input aria-label="搜索市场" value={query} onChange={e => { reset(); setQuery(e.target.value); }} placeholder="搜索技能、服务器或插件…"/></label>
      <button disabled={loading || busy} className="primary" type="submit">{loading ? '正在加载…' : '搜索'}</button>
    </form>
    {error && <div className="banner error" role="alert">{error}</div>}
    {result?.stale && <div className="banner">包含缓存结果，请留意各项获取时间。</div>}
    {result?.message && <div className="banner" role="status">{result.message}</div>}
    {result && !result.items.length && <div className="empty"><h3>没有找到匹配结果</h3><p>尝试其他关键词或来源。</p></div>}
    <div className="market-grid">{result?.items.map(item => <article className="panel market-card" key={JSON.stringify([item.source, item.name, item.version])}>
      <span className="eyebrow">{item.source}</span><h2>{item.name}</h2><p>{item.description || '来源未提供描述'}</p>
      <div className="market-meta">{item.stars !== null ? `★ ${item.stars.toLocaleString()} · GitHub 仓库` : '热度数据未提供'}{item.version && ` · v${item.version}`}</div>
      <small>获取于 {new Date(item.fetchedAt * 1000).toLocaleString()}</small>
      <a href={/^https:\/\//.test(item.url) ? item.url : undefined} target="_blank" rel="noreferrer">查看来源 <ArrowUpRight size={15}/></a>
      <button disabled={busy || loading} onClick={() => onDetail(item)}>查看详情与安装方式</button>
    </article>)}</div>
    {result?.nextCursor && pages.current < 34 && <button disabled={loading || busy} onClick={() => void search(true)}>加载更多</button>}
    {result && <p className="muted">已显示 {result.items.length} 项{pages.current >= 34 && result.nextCursor ? '，已达本次浏览上限，请缩小搜索范围。' : ''}</p>}
    {!result && !loading && !error && <div className="empty"><Store size={40}/><h3>连接三个来源，保留各自出处</h3><p>GitHub 仓库 · 官方 MCP Registry · Claude 官方插件目录</p></div>}
  </>;
}
