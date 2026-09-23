import { useState } from 'react';
import { X } from 'lucide-react';
import type { Component } from './types';

export interface EditRequest { mode: 'text' | 'package' | 'mcp'; name: string; content: string; sourcePath: string; agent: string; project: string | null }
export interface EditorSession { action: string; component: Component | null; kind: 'skill' | 'mcp'; content: string; name?: string }
export function ComponentEditor({ session, busy, error, projects = [], onClose, onPreview }: {
  session: EditorSession; busy: boolean; error?: string; projects?: string[]; onClose: () => void; onPreview: (request: EditRequest) => void;
}) {
  const [mode, setMode] = useState<EditRequest['mode']>(session.kind === 'mcp' ? 'mcp' : session.action === 'update' ? 'text' : 'package');
  const [name, setName] = useState(session.component?.name ?? session.name ?? '');
  const [content, setContent] = useState(session.content);
  const [sourcePath, setSourcePath] = useState('');
  const [agent, setAgent] = useState(session.component?.agent === 'Claude Code' ? 'Claude Code' : 'Codex');
  const [project, setProject] = useState('');
  return <div className="overlay"><section className="modal" role="dialog" aria-modal="true" aria-label="组件编辑">
    <button className="close" aria-label="关闭编辑" disabled={busy} onClick={onClose}><X /></button>
    <h2>{session.action === 'install' ? '添加' : '更新'} {session.kind === 'mcp' ? 'MCP 注册' : 'Skill'}</h2>
    {error && <p role="alert" className="banner error">{error}</p>}
    {session.kind === 'skill' && <label>内容来源<select value={mode} onChange={e => setMode(e.target.value as EditRequest['mode'])}><option value="package">本地完整 Skill 目录</option><option value="text">仅编辑 SKILL.md</option></select></label>}
    {session.kind === 'mcp' ? <p className="banner">只管理配置注册，不安装或运行服务器。更新合并所填字段，省略的字段及凭据会保留；现有密钥不会回显。</p> : <p>完整目录包含 SKILL.md、资源和脚本。预览会列出全部文件变更；脚本不会执行。仅编辑 SKILL.md 会保留其他文件。</p>}
    <label>{session.kind === 'mcp' ? '注册名称' : '目录名'}<input value={name} disabled={session.action === 'update'} onChange={e => setName(e.target.value)} placeholder="my-component" /></label>
    {(mode === 'mcp' || mode === 'package') && session.action === 'install' && <label>目标 Agent<select value={agent} onChange={e => setAgent(e.target.value)}><option>Codex</option><option>Claude Code</option></select></label>}
    {mode === 'mcp' && session.action === 'install' && <label>注册范围<select value={project} onChange={e => setProject(e.target.value)}><option value="">用户级</option>{projects.filter(Boolean).map(path => <option key={path} value={path}>项目：{path}</option>)}</select></label>}
    {mode === 'mcp' && session.component && <p>保留原注册目标：{session.component.agent} · {session.component.path}。更换目标需单独创建注册。</p>}
    {session.kind === 'skill' && session.action === 'install' && <p>{mode === 'package' && agent === 'Claude Code' ? '安装到用户级 .claude/skills 目录。' : '安装到用户级共享 .agents/skills 目录。其他 Agent 是否发现此目录取决于其配置。'}</p>}
    {mode === 'package' ? <label>已审阅的本地 Skill 目录<input value={sourcePath} onChange={e => setSourcePath(e.target.value)} placeholder="D:\\downloads\\my-skill" /></label> : <label>{mode === 'mcp' ? '单条 MCP 配置（JSON）' : 'SKILL.md'}<textarea rows={14} value={content} onChange={e => setContent(e.target.value)} spellCheck={false} /></label>}
    <button className="primary" disabled={busy || !name.trim() || !(mode === 'package' ? sourcePath.trim() : content.trim())} onClick={() => onPreview({ mode, name, content, sourcePath, agent, project: session.action === 'install' && mode === 'mcp' ? project || null : null })}>生成变更预览</button>
  </section></div>;
}
