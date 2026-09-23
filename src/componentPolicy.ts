import type { Component } from './types';

export function componentPolicy(c: Component) {
  const statuses = c.status ?? [];
  const contextual = c.bindingKind === 'inherited' || (c.kind === 'skill' && c.bindingKind === 'local') || (c.bindingKind == null && c.description?.startsWith('Project binding:'));
  const managedPath = [c.path, c.canonicalPath].some(path => /\/(synced|plugins|\.system)\//i.test((path ?? '').replaceAll('\\', '/')));
  const protectedSkill = c.kind === 'skill' && (Boolean(c.ownerId) || managedPath || ['cache', 'system', 'managed', 'synced'].includes(c.scope) || statuses.some(s => ['managed', 'plugin-owned', 'synced', 'alias'].includes(s)));
  const observed = (c.bindingKind === 'manifest' && c.kind !== 'skill') || c.scope === 'cache';
  const missing = ['missing', 'broken'].includes(c.effective) || statuses.some(s => ['missing', 'broken'].includes(s));
  const localMcp = c.kind === 'mcp' && (c.bindingKind === 'local' || c.scope === 'local');
  const readOnly = contextual || protectedSkill || observed || missing || localMcp;
  const supported = c.kind === 'mcp' ? ['Codex', 'Claude Code'].includes(c.agent) : c.kind === 'skill' && ['Codex', 'Claude Code', 'Shared'].includes(c.agent);
  return {
    edit: supported && !readOnly,
    toggle: supported && !readOnly && (c.agent === 'Codex' || (c.kind === 'skill' && c.agent === 'Shared')),
    pluginCommands: c.kind === 'plugin' && c.agent === 'Claude Code' && !contextual && !observed && !missing && (c.bindingKind == null || c.bindingKind === 'registration' || c.bindingKind === 'local'),
    reason: contextual ? '这是项目中的继承或上下文观察，请在原始注册条目管理。' : protectedSkill ? '此 Skill 由插件、Agent 或共享入口管理，不能单独修改。' : localMcp ? '本地项目 MCP 请通过 Claude Code 管理。' : observed ? '这是缓存或清单观察，不能作为独立注册修改。' : missing ? '路径缺失或失效，请修复后重新扫描。' : '此组件暂不支持直接修改，请通过所属 Agent 管理。',
  };
}

export const bindingLabels: Record<string, string> = { registration: '直接注册', inherited: '继承绑定', local: '本地项目绑定', manifest: '清单观察' };
