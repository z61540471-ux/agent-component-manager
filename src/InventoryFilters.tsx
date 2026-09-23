import { labels } from './types';

export function InventoryFilters({ scope, status, onScope, onStatus }: { scope: string; status: string; onScope: (value: string) => void; onStatus: (value: string) => void }) {
  return <><select aria-label="组件范围" value={scope} onChange={e => onScope(e.target.value)}><option value="all">所有范围</option>
    {['global', 'project', 'local', 'system', 'cache'].map(value => <option key={value} value={value}>{labels[value]}</option>)}</select>
    <select aria-label="组件状态" value={status} onChange={e => onStatus(e.target.value)}><option value="all">所有状态</option>
      {['disabled', 'shadowed', 'approval-denied', 'managed', 'missing', 'broken', 'duplicate', 'conflict', 'alias', 'drift', 'discovered', 'configured', 'installed', 'cached'].map(value => <option key={value} value={value}>{labels[value]}</option>)}</select></>;
}
