export interface Component {id:string;kind:string;name:string;description:string;agent:string;scope:string;path:string;canonicalPath:string;hash:string;version:string|null;status:string[];effective:string;source:string|null;lastSeen:number;projectPath?:string|null;bindingKind?:string|null;ownerId?:string|null}
export interface Inventory {components:Component[];issues:{path:string;message:string}[];scannedAt:number}
export interface Settings {home:string;projects:string[];codexHome?:string|null;claudeHome?:string|null}
export interface Plan {id:string;title:string;changes:{path:string;beforeHash:string|null;before:string|null;after:string|null}[];warnings:string[];createdAt:number}
export interface Operation {id:string;title:string;status:string;message:string;at:number}
export interface MarketItem {name:string;description:string;source:string;url:string;version:string|null;stars:number|null;fetchedAt:number}
export interface MarketResult {items:MarketItem[];stale:boolean;message:string|null;nextCursor?:string|null}
export interface Capabilities {mutationsEnabled:boolean;skillPackages:boolean;mcpConfiguration:boolean;githubPinnedImport:boolean;plugins:string;claudeSkillEnablement:boolean}
export interface McpDetail {name:string;version:string;description:string;config:Record<string,unknown>|null;source:string;fetchedAt:number;warnings:string[];metadata:Record<string,unknown>}
export function operationFeedback(operation:Operation):{level:'success'|'warning'|'error';message:string} {
 return {level:operation.status==='completed'?'success':['completed_with_warning','completed_with_warnings'].includes(operation.status)?'warning':'error',message:operation.message||`操作状态：${operation.status}`};
}
export const labels:Record<string,string>={duplicate:'重复内容',conflict:'同名差异',alias:'共享入口',drift:'内容变更',discovered:'已发现 · 加载未知',configured:'已配置 · 运行未知',cached:'缓存',installed:'已安装 · 启用未知',disabled:'已禁用',broken:'路径失效',global:'用户级',user:'用户级',project:'项目级',local:'本地项目',cache:'缓存',system:'系统',managed:'由 Agent / 插件管理',shadowed:'被同名注册覆盖','approval-denied':'项目批准已拒绝',missing:'文件缺失'};
export function filterComponents(items:Component[],query:string,kind:string,agent:string,scope='all',status='all'){return items.filter(c=>(kind==='all'||c.kind===kind)&&(agent==='all'||c.agent===agent)&&(scope==='all'||c.scope===scope||(scope==='global'&&c.scope==='user'))&&(status==='all'||c.effective===status||c.status?.includes(status))&&`${c.name} ${c.description} ${c.path} ${c.projectPath??''}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()))}
