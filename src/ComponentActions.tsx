import type { Component } from './types';
import { componentPolicy } from './componentPolicy';
import { PluginActions } from './PluginActions';

export function ComponentActions({ component, busy, onUpdate, onPreview, onPluginPreview }: {
  component: Component; busy: boolean; onUpdate: () => void;
  onPreview: (action: string) => void;
  onPluginPreview: (action: string, name: string, scope: string) => void;
}) {
  const policy = componentPolicy(component);
  if (policy.pluginCommands) return <PluginActions component={component} busy={busy} onPreview={onPluginPreview}/>;
  if (!policy.edit) return <p>{policy.reason}</p>;
  return <><div className="actions"><button disabled={busy} onClick={onUpdate}>更新内容</button>
    {policy.toggle && <button disabled={busy} onClick={() => onPreview(component.effective === 'disabled' ? 'enable' : 'disable')}>{component.effective === 'disabled' ? '启用预览' : '禁用预览'}</button>}
    <button disabled={busy} className="danger" onClick={() => onPreview('uninstall')}>{component.kind === 'mcp' ? '移除注册预览' : '卸载预览'}</button></div>
    <p>变更先生成预览，后端会再次检查所有权、共享路径与配置格式。</p></>;
}
