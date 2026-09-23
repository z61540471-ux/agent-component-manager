import type { Component } from './types';
import { componentPolicy } from './componentPolicy';

export function PluginActions({ component, busy, onPreview }: {
  component: Component; busy: boolean;
  onPreview: (action: string, name: string, scope: string) => void;
}) {
  const scope = component.scope === 'global' ? 'user' : component.scope;
  const supported = componentPolicy(component).pluginCommands && ['user', 'project', 'local'].includes(scope) && component.name.includes('@');
  return <><p>通过 Claude CLI 手动管理。这里只生成命令说明，不执行安装、更新或卸载，也不删除缓存。项目级命令需在对应项目目录执行。</p>
    {supported ? <div className="actions">{[['install', '安装'], ['update', '更新'], ['enable', '启用'], ['disable', '禁用'], ['uninstall', '卸载']].map(([action, label]) => <button key={action} disabled={busy} onClick={() => onPreview(action, component.name, scope)}>查看{label}命令</button>)}</div>
      : <p>缺少明确的 plugin@marketplace 标识或注册范围，无法安全生成管理命令。请使用所属 Agent 查询注册信息。</p>}
  </>;
}
