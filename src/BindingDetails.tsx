import type { Component } from './types';
import { bindingLabels } from './componentPolicy';

export function BindingDetails({ component, components }: { component: Component; components: Component[] }) {
  const owner = components.find(item => item.id === component.ownerId);
  return <dl>{component.projectPath && <><dt>项目上下文</dt><dd>{component.projectPath}</dd></>}
    {component.bindingKind && <><dt>绑定来源</dt><dd>{bindingLabels[component.bindingKind] ?? component.bindingKind}</dd></>}
    {component.ownerId && <><dt>所属插件</dt><dd>{owner?.name ?? component.ownerId}</dd></>}</dl>;
}

export function BindingSummary({ component, components }: { component: Component; components: Component[] }) {
  return <>{component.projectPath && <small title={component.projectPath}>{component.projectPath}</small>}
    {component.bindingKind && <small>{bindingLabels[component.bindingKind] ?? component.bindingKind}</small>}
    {component.ownerId && <small>所属插件：{components.find(item => item.id === component.ownerId)?.name ?? component.ownerId}</small>}</>;
}
