// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ComponentActions } from './ComponentActions';
import type { Component } from './types';
afterEach(cleanup);
it('renders no mutation buttons for a plugin-owned Skill', () => {
  render(<ComponentActions component={{ kind: 'skill', agent: 'Codex', scope: 'global', ownerId: 'parent', status: [] } as unknown as Component} busy={false} onUpdate={vi.fn()} onPreview={vi.fn()} onPluginPreview={vi.fn()}/>);
  expect(screen.queryAllByRole('button')).toHaveLength(0);
  expect(screen.getByText(/不能单独修改/)).toBeTruthy();
});
it('renders no misleading manual commands for inherited plugins', () => {
  render(<ComponentActions component={{ kind: 'plugin', name: 'example@market', agent: 'Claude Code', scope: 'project', bindingKind: 'inherited', status: [] } as unknown as Component} busy={false} onUpdate={vi.fn()} onPreview={vi.fn()} onPluginPreview={vi.fn()}/>);
  expect(screen.queryAllByRole('button')).toHaveLength(0);
});
it('keeps preview errors visible in the component drawer', () => {
  render(<ComponentActions component={{ kind: 'mcp', name: 'cua_repl', agent: 'Codex', scope: 'global', bindingKind: 'registration', effective: 'configured', status: [] } as unknown as Component} busy={false} error="MCP identity mismatch" onUpdate={vi.fn()} onPreview={vi.fn()} onPluginPreview={vi.fn()}/>);
  expect(screen.getByRole('alert').textContent).toContain('MCP identity mismatch');
});
