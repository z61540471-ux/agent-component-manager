// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { RecoveryPanel } from './RecoveryPanel';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const pending = [{ id: 'recovery-1', title: 'Restore fixture', status: 'applying', message: 'Interrupted', entries: [{ path: 'C:/fixture/config.json', before_hash: 'before', after_hash: 'after', attempted: true }] }];
it('keeps recovery disabled when native capability is unavailable', async () => {
  vi.mocked(invoke).mockResolvedValue(pending);
  render(<RecoveryPanel enabled={false} onRecovered={vi.fn()} />);
  const button = await screen.findByRole('button');
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText(/C:\/fixture\/config.json/)).toBeTruthy();
  fireEvent.click(button);
  expect(invoke).toHaveBeenCalledTimes(1);
});
it('requires review and sends only the retained recovery ID', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(pending).mockResolvedValueOnce({ status: 'rolled_back', message: 'Restored' }).mockResolvedValueOnce([]);
  const refreshed = vi.fn().mockResolvedValue(undefined);
  render(<RecoveryPanel enabled onRecovered={refreshed} />);
  const button = await screen.findByRole('button');
  expect((button as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(button);
  fireEvent.click(button);
  await waitFor(() => expect(refreshed).toHaveBeenCalledTimes(1));
  expect(invoke).toHaveBeenCalledWith('recover_operation', { id: 'recovery-1', confirmed: true });
  expect(vi.mocked(invoke).mock.calls.filter(call => call[0] === 'recover_operation')).toHaveLength(1);
  expect(screen.getByRole('status').textContent).toContain('Restored');
});
