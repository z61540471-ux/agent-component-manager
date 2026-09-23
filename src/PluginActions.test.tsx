// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PluginActions } from './PluginActions';
import { operationFeedback, type Component } from './types';
afterEach(cleanup);
it('uses installed marketplace identity and registration scope for all manual commands', () => {
  const preview = vi.fn();
  render(<PluginActions component={{ name: 'example@official', scope: 'project', kind: 'plugin', agent: 'Claude Code', bindingKind: 'registration' } as Component} busy={false} onPreview={preview} />);
  for (const [action, label] of [['install', '安装'], ['update', '更新'], ['enable', '启用'], ['disable', '禁用'], ['uninstall', '卸载']]) {
    fireEvent.click(screen.getByRole('button', { name: `查看${label}命令` }));
    expect(preview).toHaveBeenLastCalledWith(action, 'example@official', 'project');
  }
});
it('refuses to invent a registration identity for an observed cache', () => {
  render(<PluginActions component={{ name: 'example', scope: 'cache' } as Component} busy={false} onPreview={vi.fn()} />);
  expect(screen.queryAllByRole('button')).toHaveLength(0);
});
it('preserves completed-with-warning and unknown operation statuses', () => {
  const operation = { id: 'test', title: 'test', at: 0, message: 'rescan failed' };
  expect(operationFeedback({ ...operation, status: 'completed_with_warning' })).toEqual({ level: 'warning', message: 'rescan failed' });
  expect(operationFeedback({ ...operation, status: 'completed_with_warnings' })).toEqual({ level: 'warning', message: 'rescan failed' });
  expect(operationFeedback({ ...operation, status: 'recovery_required' }).level).toBe('error');
});
