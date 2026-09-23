import { expect, it } from 'vitest';
import { componentPolicy } from './componentPolicy';
import { filterComponents, type Component } from './types';

const entry: Component = { id: '1', kind: 'mcp', name: 'server', description: '', agent: 'Claude Code', scope: 'project', path: 'C:/project/.mcp.json', canonicalPath: '', hash: '', version: null, effective: 'configured', status: [], source: null, lastSeen: 0, bindingKind: 'registration', projectPath: 'C:/project' };
it('allows direct MCP registration edits while preserving agent toggle capabilities', () => {
  expect(componentPolicy(entry)).toMatchObject({ edit: true, toggle: false });
  expect(componentPolicy({ ...entry, agent: 'Codex' })).toMatchObject({ edit: true, toggle: true });
});
it.each(['inherited', 'local', 'manifest'])('refuses edits of %s MCP observations', bindingKind => {
  expect(componentPolicy({ ...entry, bindingKind }).edit).toBe(false);
});
it('keeps legacy contextual observations read-only', () => {
  expect(componentPolicy({ ...entry, bindingKind: undefined, description: 'Project binding: C:/project' }).edit).toBe(false);
});
it('allows standalone Skill manifests and does not derive typed ownership from descriptions', () => {
  const skill = { ...entry, kind: 'skill', agent: 'Codex', bindingKind: 'manifest', description: 'Project binding: example documentation' };
  expect(componentPolicy(skill)).toMatchObject({ edit: true, toggle: true });
  expect(componentPolicy({ ...skill, ownerId: 'plugin-owner' })).toMatchObject({ edit: false, toggle: false });
});
it.each([{ ownerId: 'plugin-1' }, { status: ['managed'] }, { status: ['alias'] }, { scope: 'cache' }, { scope: 'system' }])('protects skill ownership %j', fields => {
  expect(componentPolicy({ ...entry, kind: 'skill', agent: 'Codex', ...fields })).toMatchObject({ edit: false, toggle: false });
});
it('does not generate commands in an inherited plugin scope', () => {
  const plugin = { ...entry, kind: 'plugin', name: 'example@official' };
  expect(componentPolicy(plugin).pluginCommands).toBe(true);
  expect(componentPolicy({ ...plugin, bindingKind: 'inherited' }).pluginCommands).toBe(false);
  expect(componentPolicy({ ...plugin, bindingKind: 'manifest' }).pluginCommands).toBe(false);
});
it('combines project path, scope and diagnostic filters', () => {
  const items = [entry, { ...entry, id: '2', scope: 'global', status: ['shadowed'] }];
  expect(filterComponents(items, 'C:/PROJECT', 'mcp', 'Claude Code', 'project', 'configured').map(c => c.id)).toEqual(['1']);
  expect(filterComponents(items, '', 'all', 'all', 'all', 'shadowed').map(c => c.id)).toEqual(['2']);
});

it('includes Claude user registrations in the user scope filter', () => {
  const user = { ...entry, kind: 'plugin', scope: 'user' };
  expect(filterComponents([user], '', 'all', 'all', 'global')).toEqual([user]);
});
