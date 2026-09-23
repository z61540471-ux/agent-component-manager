// @vitest-environment jsdom
import { afterEach, beforeAll, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { InventoryExport } from './InventoryExport';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeAll(() => { HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); }; });
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it('requires a desktop scan before exporting', () => {
  const { rerender } = render(<InventoryExport desktop={false} scannedAt={1} home="C:/fixture" busy={false}/>);
  expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true);
  rerender(<InventoryExport desktop scannedAt={0} home="C:/fixture" busy={false}/>);
  expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true);
  expect(invoke).not.toHaveBeenCalled();
});
it('exports through native IPC once and displays the saved Unicode path', async () => {
  let resolve!: (path: string) => void;
  vi.mocked(invoke).mockImplementation(() => new Promise(r => { resolve = r as typeof resolve; }));
  render(<InventoryExport desktop scannedAt={10} home="C:/fixture" busy={false}/>);
  fireEvent.click(screen.getByRole('button', { name: '导出 JSON' }));
  fireEvent.change(screen.getByLabelText('JSON 保存路径'), { target: { value: 'D:/导出/组件.json' } });
  const button = screen.getByRole('button', { name: '保存 JSON' });
  fireEvent.click(button); fireEvent.click(button);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('export_inventory', { destination: 'D:/导出/组件.json' });
  expect((screen.getByRole('button', { name: '关闭导出' }) as HTMLButtonElement).disabled).toBe(true);
  resolve('D:/导出/组件.json');
  await waitFor(() => expect(screen.getByRole('status').textContent).toContain('D:/导出/组件.json'));
});
it('keeps a failed export visible and lets the user choose another destination', async () => {
  vi.mocked(invoke).mockRejectedValueOnce('File already exists').mockResolvedValueOnce('D:/new.json');
  render(<InventoryExport desktop scannedAt={10} home="C:/fixture" busy={false}/>);
  fireEvent.click(screen.getByRole('button', { name: '导出 JSON' }));
  fireEvent.change(screen.getByLabelText('JSON 保存路径'), { target: { value: 'relative.json' } });
  fireEvent.click(screen.getByRole('button', { name: '保存 JSON' }));
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText('JSON 保存路径'), { target: { value: 'D:/existing.json' } });
  fireEvent.click(screen.getByRole('button', { name: '保存 JSON' }));
  expect((await screen.findByRole('alert')).textContent).toContain('File already exists');
  fireEvent.change(screen.getByLabelText('JSON 保存路径'), { target: { value: 'D:/new.json' } });
  fireEvent.click(screen.getByRole('button', { name: '保存 JSON' }));
  await waitFor(() => expect(screen.getByRole('status').textContent).toContain('D:/new.json'));
});
