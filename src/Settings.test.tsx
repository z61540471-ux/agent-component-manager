// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { App } from './main';
import type { Component } from './types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it('saves custom roots and uses null to restore automatic detection', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'settings') return { home: 'C:/fixture', projects: [], codexHome: null, claudeHome: 'C:/old' };
    if (command === 'inventory') return { components: [], issues: [], scannedAt: 10 };
    if (command === 'capabilities') return { mutationsEnabled: true };
    return undefined;
  });
  render(<App/>);
  fireEvent.click(screen.getByRole('button', { name: '设置' }));
  await waitFor(() => expect((screen.getByLabelText('用户主目录') as HTMLInputElement).value).toBe('C:/fixture'));
  fireEvent.change(screen.getByLabelText('Codex 目录（可选）'), { target: { value: ' D:/自定义/codex ' } });
  fireEvent.change(screen.getByLabelText('Claude Code 目录（可选）'), { target: { value: '  ' } });
  fireEvent.click(screen.getByRole('button', { name: '保存设置' }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('save_settings', { value: { home: 'C:/fixture', projects: [], codexHome: 'D:/自定义/codex', claudeHome: null } }));
  expect((await screen.findByRole('status')).textContent).toContain('设置已保存');
});
it('clears selected component and retained preview when rescanning', async () => {
  const component = { id: 'fixture', name: 'fixture-server', kind: 'mcp', agent: 'Codex', scope: 'global', bindingKind: 'registration', description: '', path: 'C:/config.toml', canonicalPath: 'C:/config.toml', hash: '123', effective: 'configured', status: [], version: null, source: null, lastSeen: 10 } as Component;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'settings') return { home: 'C:/fixture', projects: [] };
    if (command === 'inventory') return { components: [component], issues: [], scannedAt: 10 };
    if (command === 'capabilities') return { mutationsEnabled: true };
    if (command === 'scan') return { components: [], issues: [], scannedAt: 20 };
    if (command === 'preview_mcp') return { id: 'old-plan', title: 'Old preview', changes: [], warnings: [], createdAt: 10 };
    return undefined;
  });
  render(<App/>);
  fireEvent.click(await screen.findByRole('button', { name: 'fixture-server' }));
  expect(screen.getByRole('button', { name: '关闭详情' })).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: '移除注册预览' }));
  await screen.findByText('Old preview');
  fireEvent.click(screen.getByRole('button', { name: '扫描本机' }));
  await waitFor(() => expect(screen.queryByText('Old preview')).toBeNull());
  expect(screen.queryByRole('button', { name: '关闭详情' })).toBeNull();
});
