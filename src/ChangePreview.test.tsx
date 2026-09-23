// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ChangePreview } from './ChangePreview';
import type { Plan } from './types';

afterEach(cleanup);
const plan: Plan = { id: 'private-plan-id', title: 'Install example', createdAt: 1, warnings: ['Back up first'], changes: [{ path: 'C:/skills/example/SKILL.md', beforeHash: null, before: null, after: 'Example' }] };
describe('change confirmation', () => {
  it('keeps manual instructions separate from executable changes', () => {
    render(<ChangePreview plan={{ ...plan, changes: [] }} enabled busy={false} onClose={() => {}} onApply={vi.fn()} />);
    expect(screen.queryByRole('checkbox')).toBeNull();
    expect(screen.queryByRole('button', { name: '备份并应用' })).toBeNull();
    expect(screen.getByText(/手动命令需在所属 Agent 中执行/)).toBeTruthy();
  });
  it('requires confirmation and submits only the backend plan ID', async () => {
    const apply = vi.fn().mockResolvedValue(undefined);
    render(<ChangePreview plan={plan} enabled busy={false} onClose={() => {}} onApply={apply} />);
    const button = screen.getByRole('button', { name: '备份并应用' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(apply).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(button);
    await waitFor(() => expect(apply).toHaveBeenCalledExactlyOnceWith('private-plan-id'));
    await waitFor(() => expect((screen.getByRole('checkbox') as HTMLInputElement).checked).toBe(false));
  });
  it('never writes while native verification is gated, even after confirmation', () => {
    const apply = vi.fn();
    render(<ChangePreview plan={plan} enabled={false} busy={false} onClose={() => {}} onApply={apply} />);
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByRole('button', { name: '备份并应用' }));
    expect(apply).not.toHaveBeenCalled();
  });
  it('blocks repeated apply and dismissal while an operation is pending', async () => {
    let finish!: () => void;
    const apply = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
    const close = vi.fn();
    render(<ChangePreview plan={plan} enabled busy={false} onClose={close} onApply={apply} />);
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByRole('button', { name: '备份并应用' }));
    fireEvent.click(screen.getByRole('button', { name: '正在应用…' }));
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }));
    expect(apply).toHaveBeenCalledTimes(1);
    expect(close).not.toHaveBeenCalled();
    finish();
    await waitFor(() => expect(screen.getByRole('button', { name: '备份并应用' })).toBeTruthy());
  });
});
