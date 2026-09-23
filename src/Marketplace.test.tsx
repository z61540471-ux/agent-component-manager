// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { Marketplace } from './Marketplace';
import type { MarketResult } from './types';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const page = (name: string, nextCursor: string | null = null): MarketResult => ({ items: [{ name, description: '', source: 'GitHub', version: null, stars: 2, url: 'https://github.com/example/repo', fetchedAt: 1 }], stale: false, message: null, nextCursor });
const mount = () => render(<Marketplace desktop busy={false} onDetail={vi.fn()}/>);
it('appends distinct entries and requests only the returned next cursor', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(page('first', '2')).mockResolvedValueOnce({ ...page('second'), items: [...page('first').items, ...page('second').items] });
  mount(); fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  fireEvent.click(await screen.findByRole('button', { name: '加载更多' }));
  await screen.findByText('second');
  expect(screen.getAllByText('first')).toHaveLength(1);
  expect(invoke).toHaveBeenLastCalledWith('market', { source: 'GitHub', query: '', cursor: '2' });
  expect(screen.queryByRole('button', { name: '加载更多' })).toBeNull();
});
it('ignores outdated results after switching source and prevents duplicate submission', async () => {
  let resolve!: (value: MarketResult) => void;
  vi.mocked(invoke).mockReturnValue(new Promise(r => { resolve = r; }));
  mount(); const button = screen.getByRole('button', { name: '搜索' });
  fireEvent.click(button); fireEvent.submit(button.closest('form')!);
  expect(invoke).toHaveBeenCalledTimes(1);
  fireEvent.change(screen.getByLabelText('市场来源'), { target: { value: 'MCP Registry' } });
  await act(async () => resolve(page('old')));
  expect(screen.queryByText('old')).toBeNull();
  expect(screen.getByRole('button', { name: '搜索' }).hasAttribute('disabled')).toBe(false);
});
it('clears results when the query changes and shows cached failure diagnostics', async () => {
  vi.mocked(invoke).mockResolvedValue({ ...page('cached'), stale: true, message: 'GitHub rate limit reached' });
  mount(); fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByText('cached');
  expect(screen.getByText(/包含缓存结果/)).toBeTruthy();
  expect(screen.getByText('GitHub rate limit reached')).toBeTruthy();
  fireEvent.change(screen.getByLabelText('搜索市场'), { target: { value: 'other' } });
  expect(screen.queryByText('cached')).toBeNull();
});
it('keeps prior results on load-more failure for retry', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(page('first', '2')).mockRejectedValueOnce('offline');
  mount(); fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  fireEvent.click(await screen.findByRole('button', { name: '加载更多' }));
  await screen.findByRole('alert');
  expect(screen.getByText('first')).toBeTruthy();
  expect(screen.getByRole('button', { name: '加载更多' }).hasAttribute('disabled')).toBe(false);
});
