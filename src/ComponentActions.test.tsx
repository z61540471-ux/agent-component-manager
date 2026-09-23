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
