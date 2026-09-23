// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ComponentEditor } from './ComponentEditor';
import type { Component } from './types';
afterEach(cleanup);
it('loads existing Skill content instead of a default template', () => {
  const preview = vi.fn();
  render(<ComponentEditor session={{ action: 'update', component: null, kind: 'skill', name: 'original', content: 'Existing instructions' }} busy={false} onClose={() => {}} onPreview={preview} />);
  expect((screen.getByRole('textbox', { name: 'SKILL.md' }) as HTMLTextAreaElement).value).toBe('Existing instructions');
  fireEvent.click(screen.getByRole('button', { name: '生成变更预览' }));
  expect(preview).toHaveBeenCalledWith(expect.objectContaining({ mode: 'text', content: 'Existing instructions' }));
});
it('passes a selected configured project only for new MCP registrations', () => {
  const preview = vi.fn();
  render(<ComponentEditor projects={['D:/projects/demo']} session={{ action: 'install', component: null, kind: 'mcp', name: 'example', content: '{"url":"https://example.org/mcp"}' }} busy={false} onClose={() => {}} onPreview={preview} />);
  fireEvent.change(screen.getByRole('combobox', { name: '注册范围' }), { target: { value: 'D:/projects/demo' } });
  fireEvent.click(screen.getByRole('button', { name: '生成变更预览' }));
  expect(preview).toHaveBeenCalledWith(expect.objectContaining({ project: 'D:/projects/demo', agent: 'Codex' }));
});
it('locks the existing MCP agent and target when updating', () => {
  const component = { name: 'example', agent: 'Claude Code', path: 'D:/projects/demo/.mcp.json' } as Component;
  render(<ComponentEditor projects={['D:/other']} session={{ action: 'update', component, kind: 'mcp', content: '{}' }} busy={false} onClose={() => {}} onPreview={vi.fn()} />);
  expect(screen.queryByRole('combobox', { name: '注册范围' })).toBeNull();
  expect(screen.queryByRole('combobox', { name: '目标 Agent' })).toBeNull();
  expect(screen.getByText(/D:\/projects\/demo\/.mcp.json/)).toBeTruthy();
});
it('requires a local package path and passes the entire package request to preview', () => {
  const preview = vi.fn();
  render(<ComponentEditor session={{ action: 'install', component: null, kind: 'skill', content: '' }} busy={false} onClose={() => {}} onPreview={preview} />);
  const button = screen.getByRole('button', { name: '生成变更预览' }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.change(screen.getByRole('textbox', { name: '目录名' }), { target: { value: 'example' } });
  fireEvent.change(screen.getByRole('textbox', { name: '已审阅的本地 Skill 目录' }), { target: { value: 'D:/package/example' } });
  fireEvent.click(button);
  expect(preview).toHaveBeenCalledWith(expect.objectContaining({ mode: 'package', name: 'example', sourcePath: 'D:/package/example' }));
});
