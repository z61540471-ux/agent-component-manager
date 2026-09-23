// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MarketDetail } from './MarketDetail';
import type { MarketItem } from './types';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(cleanup);
const item: MarketItem = { name: 'owner/repo', description: 'Example', source: 'GitHub', url: 'https://github.com/owner/repo', stars: 5, version: null, fetchedAt: 1 };
it('resolves a selected repository folder through a preview rather than applying', () => {
  const preview = vi.fn();
  render(<MarketDetail item={item} busy={false} onClose={() => {}} onMcp={() => {}} onPreview={preview} />);
  fireEvent.change(screen.getByRole('textbox', { name: '安装目录名' }), { target: { value: 'example' } });
  fireEvent.change(screen.getByRole('textbox', { name: '仓库内 Skill 目录（留空表示根目录）' }), { target: { value: 'skills/example' } });
  fireEvent.click(screen.getByRole('button', { name: '解析来源并预览安装' }));
  expect(preview).toHaveBeenCalledExactlyOnceWith('preview_github', { repository: 'owner/repo', revision: 'main', subdirectory: 'skills/example', name: 'example', action: 'install', id: null });
});
it('loads pinned MCP detail and requires review before seeding its URL', async () => {
  vi.mocked(invoke).mockResolvedValue({ name: 'example/server', version: '1.2', config: { url: 'https://example.org/mcp' }, fetchedAt: 1, metadata: { remotes: [{ type: 'streamable-http', url: 'https://example.org/mcp', headers: [{ name: 'Authorization', isRequired: true }] }] } });
  const onMcp = vi.fn();
  render(<MarketDetail item={{ ...item, name: 'example/server', source: 'MCP Registry', version: '1.2' }} busy={false} onClose={() => {}} onMcp={onMcp} onPreview={vi.fn()} />);
  const button = await screen.findByRole('button', { name: '使用此地址编辑注册配置' });
  expect(invoke).toHaveBeenCalledWith('mcp_detail', { name: 'example/server', version: '1.2' });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText(/Authorization/)).toBeTruthy();
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(button);
  expect(JSON.parse(onMcp.mock.calls[0][0])).toEqual({ url: 'https://example.org/mcp' });
});
it('does not fabricate a URL for package-only metadata', async () => {
  vi.mocked(invoke).mockResolvedValue({ name: 'example/server', version: '1', config: null, fetchedAt: 1, metadata: { packages: [{ identifier: 'example-package', environmentVariables: [{ name: 'API_KEY', isRequired: true }] }] } });
  const onMcp = vi.fn();
  render(<MarketDetail item={{ ...item, source: 'MCP Registry' }} busy={false} onClose={() => {}} onMcp={onMcp} onPreview={vi.fn()} />);
  fireEvent.click(await screen.findByRole('button', { name: '手动填写 MCP 注册配置' }));
  expect(onMcp).toHaveBeenCalledWith('{}');
  expect(screen.queryByRole('button', { name: '使用此地址编辑注册配置' })).toBeNull();
});
it('exposes plugin instructions without an automatic install button', () => {
  render(<MarketDetail item={{ ...item, source: 'Claude Marketplace' }} busy={false} onClose={() => {}} onMcp={() => {}} onPreview={vi.fn()} />);
  expect(screen.getByText(/不会执行命令或修改插件缓存/)).toBeTruthy();
  expect(screen.queryByRole('button', { name: '解析来源并预览安装' })).toBeNull();
});
