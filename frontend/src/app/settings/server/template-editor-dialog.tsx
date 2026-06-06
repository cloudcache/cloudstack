'use client'

import { useState } from "react";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";

// Editable shape carried around in the editor dialog.
// Mirrors TemplateDto + drops server-managed fields, adds form ergonomics.
export interface EditableTemplate {
    id?: string;
    slug: string;
    name: string;
    icon_url: string;
    category: 'app' | 'database';
    description: string;
    is_active: boolean;
    image_registry_id: string;
    image_repository: string;
    image_tag: string;
    image_digest: string;
    spec: any;            // { appModel, appPorts, appVolumes, appFileMounts, appDomains }
    requirements: Requirement[];
    inputs: InputEntry[];
}

interface Requirement {
    key: string;
    kind: 'database' | 'cache' | 'objstore' | 'mq' | 'smtp';
    engine?: string;
    label?: string;
    env_mapping?: Record<string, string>;
    config_files?: Array<{ path: string; template: string }>;
    binding_modes?: Array<'managed' | 'provision'>;
}

interface InputEntry {
    key: string;
    label: string;
    value: any;
    isEnvVar: boolean;
    randomGeneratedIfEmpty: boolean;
}

interface Port { port: number; protocol?: string }
interface AppModelShape {
    name?: string;
    appType?: string;            // APP | DATABASE | INTERNAL
    sourceType?: string;         // CONTAINER | GIT | etc
    replicas?: number;
    envVars?: string;            // multi-line KEY=VALUE
    ingressNetworkPolicy?: string;
    egressNetworkPolicy?: string;
    useNetworkPolicy?: boolean;
    healthCheckPeriodSeconds?: number;
    healthCheckTimeoutSeconds?: number;
    healthCheckFailureThreshold?: number;
}

export function newBlankTemplate(): EditableTemplate {
    return {
        slug: '',
        name: '',
        icon_url: '',
        category: 'app',
        description: '',
        is_active: true,
        image_registry_id: '',
        image_repository: '',
        image_tag: 'latest',
        image_digest: '',
        spec: {
            appModel: {
                name: '',
                appType: 'APP',
                sourceType: 'CONTAINER',
                replicas: 1,
                envVars: '',
                ingressNetworkPolicy: 'ALLOW_ALL',
                egressNetworkPolicy: 'ALLOW_ALL',
                useNetworkPolicy: true,
                healthCheckPeriodSeconds: 15,
                healthCheckTimeoutSeconds: 10,
                healthCheckFailureThreshold: 3,
            },
            appPorts: [],
            appVolumes: [],
            appFileMounts: [],
            appDomains: [],
        },
        requirements: [],
        inputs: [],
    };
}

export default function TemplateEditorDialog({
    template, onClose, onSave,
}: {
    template: EditableTemplate;
    onClose: () => void;
    onSave: (t: EditableTemplate) => Promise<boolean>;
}) {
    const [t, setT] = useState<EditableTemplate>(template);
    const [saving, setSaving] = useState(false);
    const isCreating = !template.id;

    const updateAppModel = (patch: Partial<AppModelShape>) =>
        setT(s => ({ ...s, spec: { ...s.spec, appModel: { ...s.spec.appModel, ...patch } } }));

    const submit = async () => {
        if (!t.slug || !t.name || !t.image_repository) {
            toast.error("slug, name and image_repository are required.");
            return;
        }
        setSaving(true);
        const ok = await onSave(t);
        setSaving(false);
        if (!ok) toast.error("Failed to save template.");
    };

    return (
        <Dialog open={true} onOpenChange={(v) => !v && onClose()}>
            <DialogContent className="sm:max-w-[900px] max-h-[90vh] overflow-hidden flex flex-col">
                <DialogHeader>
                    <DialogTitle>{isCreating ? 'Create template' : `Edit ${template.name}`}</DialogTitle>
                </DialogHeader>

                <Tabs defaultValue="basic" className="flex-1 overflow-hidden flex flex-col">
                    <TabsList className="grid grid-cols-5 mb-3">
                        <TabsTrigger value="basic">Basic</TabsTrigger>
                        <TabsTrigger value="image">Image</TabsTrigger>
                        <TabsTrigger value="runtime">Runtime</TabsTrigger>
                        <TabsTrigger value="requirements">
                            Dependencies {t.requirements.length > 0 && <Badge variant="secondary" className="ml-2">{t.requirements.length}</Badge>}
                        </TabsTrigger>
                        <TabsTrigger value="inputs">
                            Inputs {t.inputs.length > 0 && <Badge variant="secondary" className="ml-2">{t.inputs.length}</Badge>}
                        </TabsTrigger>
                    </TabsList>

                    <div className="overflow-y-auto flex-1 pr-2">
                        {/* ── Basic ── */}
                        <TabsContent value="basic" className="space-y-4">
                            <Row>
                                <Field label="Slug (immutable identifier)" required>
                                    <Input value={t.slug} onChange={e => setT(s => ({ ...s, slug: e.target.value }))} placeholder="my-app" />
                                </Field>
                                <Field label="Name" required>
                                    <Input value={t.name} onChange={e => setT(s => ({ ...s, name: e.target.value }))} placeholder="My App" />
                                </Field>
                            </Row>
                            <Row>
                                <Field label="Category">
                                    <Select value={t.category} onValueChange={(v: any) => setT(s => ({ ...s, category: v }))}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="app">App</SelectItem>
                                            <SelectItem value="database">Database</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </Field>
                                <Field label="Icon URL">
                                    <Input value={t.icon_url} onChange={e => setT(s => ({ ...s, icon_url: e.target.value }))} placeholder="https://cdn.simpleicons.org/…" />
                                </Field>
                            </Row>
                            <Field label="Description">
                                <Textarea value={t.description} onChange={e => setT(s => ({ ...s, description: e.target.value }))} rows={3} />
                            </Field>
                            <div className="flex items-center justify-between">
                                <Label>Active</Label>
                                <Switch checked={t.is_active} onCheckedChange={v => setT(s => ({ ...s, is_active: v }))} />
                            </div>
                        </TabsContent>

                        {/* ── Image ── */}
                        <TabsContent value="image" className="space-y-4">
                            <Field label="Registry ID (UUID)" hint="Leave blank to use the default registry (Docker Hub).">
                                <Input value={t.image_registry_id} onChange={e => setT(s => ({ ...s, image_registry_id: e.target.value }))} placeholder="optional" />
                            </Field>
                            <Row>
                                <Field label="Repository" required>
                                    <Input value={t.image_repository} onChange={e => setT(s => ({ ...s, image_repository: e.target.value }))} placeholder="library/nginx" className="font-mono" />
                                </Field>
                                <Field label="Tag">
                                    <Input value={t.image_tag} onChange={e => setT(s => ({ ...s, image_tag: e.target.value }))} placeholder="latest" className="font-mono" />
                                </Field>
                            </Row>
                            <Field label="Digest (optional, pins image immutably)">
                                <Input value={t.image_digest} onChange={e => setT(s => ({ ...s, image_digest: e.target.value }))} placeholder="sha256:…" className="font-mono" />
                            </Field>
                            <div className="text-xs text-muted-foreground p-2 bg-muted rounded">
                                Preview: <span className="font-mono">{t.image_repository || '<repo>'}:{t.image_tag}{t.image_digest ? `@${t.image_digest}` : ''}</span>
                            </div>
                        </TabsContent>

                        {/* ── Runtime (appModel + ports + envVars) ── */}
                        <TabsContent value="runtime" className="space-y-4">
                            <Row>
                                <Field label="Replicas">
                                    <Input type="number" min={0} value={t.spec.appModel?.replicas ?? 1}
                                        onChange={e => updateAppModel({ replicas: Number(e.target.value) })} />
                                </Field>
                                <Field label="App Type">
                                    <Select value={t.spec.appModel?.appType ?? 'APP'} onValueChange={v => updateAppModel({ appType: v })}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="APP">APP</SelectItem>
                                            <SelectItem value="DATABASE">DATABASE</SelectItem>
                                            <SelectItem value="INTERNAL">INTERNAL</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </Field>
                            </Row>
                            <Field label="Built-in env vars" hint="One KEY=VALUE per line. Merged with requirement-injected env vars at deploy time.">
                                <Textarea
                                    rows={6}
                                    className="font-mono text-xs"
                                    value={t.spec.appModel?.envVars ?? ''}
                                    onChange={e => updateAppModel({ envVars: e.target.value })}
                                />
                            </Field>

                            <PortsEditor
                                ports={t.spec.appPorts ?? []}
                                onChange={(ports) => setT(s => ({ ...s, spec: { ...s.spec, appPorts: ports } }))} />

                            <Row>
                                <Field label="Ingress network policy">
                                    <Select value={t.spec.appModel?.ingressNetworkPolicy ?? 'ALLOW_ALL'}
                                        onValueChange={v => updateAppModel({ ingressNetworkPolicy: v })}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="ALLOW_ALL">ALLOW_ALL</SelectItem>
                                            <SelectItem value="DENY_ALL">DENY_ALL</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </Field>
                                <Field label="Egress network policy">
                                    <Select value={t.spec.appModel?.egressNetworkPolicy ?? 'ALLOW_ALL'}
                                        onValueChange={v => updateAppModel({ egressNetworkPolicy: v })}>
                                        <SelectTrigger><SelectValue /></SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="ALLOW_ALL">ALLOW_ALL</SelectItem>
                                            <SelectItem value="DENY_ALL">DENY_ALL</SelectItem>
                                        </SelectContent>
                                    </Select>
                                </Field>
                            </Row>
                            <Row>
                                <Field label="Health-check period (s)">
                                    <Input type="number" min={1} value={t.spec.appModel?.healthCheckPeriodSeconds ?? 15}
                                        onChange={e => updateAppModel({ healthCheckPeriodSeconds: Number(e.target.value) })} />
                                </Field>
                                <Field label="Health-check timeout (s)">
                                    <Input type="number" min={1} value={t.spec.appModel?.healthCheckTimeoutSeconds ?? 10}
                                        onChange={e => updateAppModel({ healthCheckTimeoutSeconds: Number(e.target.value) })} />
                                </Field>
                                <Field label="Failure threshold">
                                    <Input type="number" min={1} value={t.spec.appModel?.healthCheckFailureThreshold ?? 3}
                                        onChange={e => updateAppModel({ healthCheckFailureThreshold: Number(e.target.value) })} />
                                </Field>
                            </Row>
                        </TabsContent>

                        {/* ── Requirements ── */}
                        <TabsContent value="requirements">
                            <RequirementsEditor
                                requirements={t.requirements}
                                onChange={(reqs) => setT(s => ({ ...s, requirements: reqs }))} />
                        </TabsContent>

                        {/* ── Inputs ── */}
                        <TabsContent value="inputs">
                            <InputsEditor
                                inputs={t.inputs}
                                onChange={(inps) => setT(s => ({ ...s, inputs: inps }))} />
                        </TabsContent>
                    </div>
                </Tabs>

                <DialogFooter>
                    <Button variant="outline" onClick={onClose}>Cancel</Button>
                    <Button onClick={submit} disabled={saving}>{saving ? 'Saving…' : (isCreating ? 'Create' : 'Save')}</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}

// ── Small layout helpers ───────────────────────────────────────────────────────

function Row({ children }: { children: React.ReactNode }) {
    return <div className="grid grid-cols-1 md:grid-cols-2 gap-3">{children}</div>;
}
function Field({ label, hint, required, children }: { label: string; hint?: string; required?: boolean; children: React.ReactNode }) {
    return (
        <div className="space-y-1">
            <Label>{label}{required && <span className="text-destructive ml-1">*</span>}</Label>
            {children}
            {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
        </div>
    );
}

// ── Ports editor ─────────────────────────────────────────────────────────────

function PortsEditor({ ports, onChange }: { ports: Port[]; onChange: (p: Port[]) => void }) {
    return (
        <div className="space-y-2">
            <div className="flex items-center justify-between">
                <Label>Container ports</Label>
                <Button size="sm" variant="outline" onClick={() => onChange([...ports, { port: 8080 }])}>
                    <Plus className="h-3 w-3 mr-1" />Add port
                </Button>
            </div>
            <div className="space-y-2">
                {ports.map((p, i) => (
                    <div key={i} className="flex items-center gap-2">
                        <Input
                            type="number"
                            min={1} max={65535}
                            value={p.port}
                            onChange={e => onChange(ports.map((pp, idx) => idx === i ? { ...pp, port: Number(e.target.value) } : pp))}
                            className="w-32 font-mono" />
                        <Select value={p.protocol ?? 'TCP'} onValueChange={v => onChange(ports.map((pp, idx) => idx === i ? { ...pp, protocol: v } : pp))}>
                            <SelectTrigger className="w-24"><SelectValue /></SelectTrigger>
                            <SelectContent>
                                <SelectItem value="TCP">TCP</SelectItem>
                                <SelectItem value="UDP">UDP</SelectItem>
                            </SelectContent>
                        </Select>
                        <Button size="sm" variant="ghost" onClick={() => onChange(ports.filter((_, idx) => idx !== i))}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                    </div>
                ))}
                {ports.length === 0 && <p className="text-xs text-muted-foreground">No ports declared.</p>}
            </div>
        </div>
    );
}

// ── Requirements editor ──────────────────────────────────────────────────────

const REQUIREMENT_KINDS: Array<{ value: Requirement['kind']; label: string; defaultModes: Array<'managed' | 'provision'> }> = [
    { value: 'database', label: 'Database', defaultModes: ['managed', 'provision'] },
    { value: 'cache', label: 'Redis cache', defaultModes: ['managed'] },
    { value: 'objstore', label: 'Object storage (S3)', defaultModes: ['managed'] },
    { value: 'mq', label: 'Message queue', defaultModes: ['managed'] },
    { value: 'smtp', label: 'SMTP relay', defaultModes: ['managed'] },
];

function RequirementsEditor({ requirements, onChange }: { requirements: Requirement[]; onChange: (r: Requirement[]) => void }) {
    const addReq = () => onChange([...requirements, {
        key: 'dep',
        kind: 'database',
        engine: 'mysql',
        label: '',
        env_mapping: { host: 'DB_HOST', port: 'DB_PORT', user: 'DB_USER', password: 'DB_PASS', name: 'DB_NAME' },
        config_files: [],
        binding_modes: ['managed', 'provision'],
    }]);
    const update = (i: number, patch: Partial<Requirement>) =>
        onChange(requirements.map((r, idx) => idx === i ? { ...r, ...patch } : r));
    const remove = (i: number) => onChange(requirements.filter((_, idx) => idx !== i));

    return (
        <div className="space-y-3">
            <div className="flex items-center justify-between">
                <p className="text-sm text-muted-foreground">
                    Once a requirement is declared, deployers must bind it to a managed or provisioned service.
                </p>
                <Button size="sm" variant="outline" onClick={addReq}>
                    <Plus className="h-3 w-3 mr-1" />Add dependency
                </Button>
            </div>
            {requirements.length === 0 && (
                <p className="text-xs text-muted-foreground italic">No service dependencies declared.</p>
            )}
            {requirements.map((req, i) => (
                <div key={i} className="border rounded-lg p-3 space-y-3">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Badge variant="outline">{req.kind}</Badge>
                            <span className="font-medium">{req.label || req.key}</span>
                        </div>
                        <Button size="sm" variant="ghost" onClick={() => remove(i)}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                    </div>
                    <Row>
                        <Field label="Key (env prefix)">
                            <Input value={req.key} onChange={e => update(i, { key: e.target.value })} placeholder="db" />
                        </Field>
                        <Field label="Label (shown in deploy dialog)">
                            <Input value={req.label ?? ''} onChange={e => update(i, { label: e.target.value })} placeholder="Primary database" />
                        </Field>
                    </Row>
                    <Row>
                        <Field label="Kind">
                            <Select value={req.kind} onValueChange={(v: any) => update(i, { kind: v })}>
                                <SelectTrigger><SelectValue /></SelectTrigger>
                                <SelectContent>
                                    {REQUIREMENT_KINDS.map(k => <SelectItem key={k.value} value={k.value}>{k.label}</SelectItem>)}
                                </SelectContent>
                            </Select>
                        </Field>
                        <Field label="Engine (filter)" hint="Used to narrow the list of bindable services (e.g. 'mysql', 'postgres', 'rabbitmq').">
                            <Input value={req.engine ?? ''} onChange={e => update(i, { engine: e.target.value })} placeholder="mysql" />
                        </Field>
                    </Row>
                    <EnvMappingEditor mapping={req.env_mapping ?? {}} onChange={(m) => update(i, { env_mapping: m })} />
                    <ConfigFilesEditor files={req.config_files ?? []} onChange={(f) => update(i, { config_files: f })} />
                </div>
            ))}
        </div>
    );
}

function EnvMappingEditor({ mapping, onChange }: { mapping: Record<string, string>; onChange: (m: Record<string, string>) => void }) {
    const rows = Object.entries(mapping);
    return (
        <div className="space-y-2">
            <div className="flex items-center justify-between">
                <Label className="text-xs uppercase tracking-wide">Env-var injection</Label>
                <Button size="sm" variant="ghost" onClick={() => onChange({ ...mapping, '': '' })}>
                    <Plus className="h-3 w-3 mr-1" />Add
                </Button>
            </div>
            {rows.length === 0 && <p className="text-xs text-muted-foreground italic pl-1">No env vars injected.</p>}
            <div className="space-y-1">
                {rows.map(([logical, envKey], i) => (
                    <div key={i} className="flex items-center gap-2">
                        <Input className="font-mono text-xs flex-1"
                            placeholder="logical (host, port, password)"
                            value={logical}
                            onChange={(e) => {
                                const next: Record<string, string> = {};
                                rows.forEach(([k, v], idx) => next[idx === i ? e.target.value : k] = v);
                                onChange(next);
                            }} />
                        <span className="text-muted-foreground">→</span>
                        <Input className="font-mono text-xs flex-1"
                            placeholder="ENV_KEY"
                            value={envKey}
                            onChange={(e) => onChange({ ...mapping, [logical]: e.target.value })} />
                        <Button size="sm" variant="ghost" onClick={() => {
                            const next = { ...mapping }; delete next[logical]; onChange(next);
                        }}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                    </div>
                ))}
            </div>
        </div>
    );
}

function ConfigFilesEditor({ files, onChange }: { files: Array<{ path: string; template: string }>; onChange: (f: Array<{ path: string; template: string }>) => void }) {
    return (
        <div className="space-y-2">
            <div className="flex items-center justify-between">
                <Label className="text-xs uppercase tracking-wide">Config files (minijinja)</Label>
                <Button size="sm" variant="ghost" onClick={() => onChange([...files, { path: '', template: '' }])}>
                    <Plus className="h-3 w-3 mr-1" />Add file
                </Button>
            </div>
            {files.length === 0 && <p className="text-xs text-muted-foreground italic pl-1">No config files generated.</p>}
            {files.map((f, i) => (
                <div key={i} className="border rounded p-2 space-y-2">
                    <div className="flex items-center gap-2">
                        <Input className="font-mono text-xs"
                            placeholder="/etc/app/db.yml"
                            value={f.path}
                            onChange={e => onChange(files.map((ff, idx) => idx === i ? { ...ff, path: e.target.value } : ff))} />
                        <Button size="sm" variant="ghost" onClick={() => onChange(files.filter((_, idx) => idx !== i))}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                    </div>
                    <Textarea
                        className="font-mono text-xs"
                        rows={5}
                        placeholder={'host: {{ host }}\nuser: {{ user }}\npassword: {{ password }}'}
                        value={f.template}
                        onChange={e => onChange(files.map((ff, idx) => idx === i ? { ...ff, template: e.target.value } : ff))} />
                </div>
            ))}
        </div>
    );
}

// ── Inputs editor ────────────────────────────────────────────────────────────

function InputsEditor({ inputs, onChange }: { inputs: InputEntry[]; onChange: (i: InputEntry[]) => void }) {
    const add = () => onChange([...inputs, { key: '', label: '', value: '', isEnvVar: true, randomGeneratedIfEmpty: false }]);
    const update = (i: number, patch: Partial<InputEntry>) =>
        onChange(inputs.map((it, idx) => idx === i ? { ...it, ...patch } : it));

    return (
        <div className="space-y-3">
            <div className="flex items-center justify-between">
                <p className="text-sm text-muted-foreground">
                    Per-deploy form fields. If <code>isEnvVar</code> is on, the value becomes an env var on the container.
                </p>
                <Button size="sm" variant="outline" onClick={add}>
                    <Plus className="h-3 w-3 mr-1" />Add input
                </Button>
            </div>
            {inputs.length === 0 && <p className="text-xs text-muted-foreground italic">No additional inputs.</p>}
            {inputs.map((inp, i) => (
                <div key={i} className="border rounded p-3 space-y-2">
                    <Row>
                        <Field label="Key">
                            <Input className="font-mono text-xs" value={inp.key} onChange={e => update(i, { key: e.target.value })} placeholder="ADMIN_PASSWORD" />
                        </Field>
                        <Field label="Label">
                            <Input value={inp.label} onChange={e => update(i, { label: e.target.value })} placeholder="Admin password" />
                        </Field>
                    </Row>
                    <Field label="Default value">
                        <Input value={String(inp.value ?? '')} onChange={e => update(i, { value: e.target.value })} />
                    </Field>
                    <Row>
                        <div className="flex items-center justify-between">
                            <Label>Inject as env var</Label>
                            <Switch checked={inp.isEnvVar} onCheckedChange={v => update(i, { isEnvVar: v })} />
                        </div>
                        <div className="flex items-center justify-between">
                            <Label>Auto-generate if empty</Label>
                            <Switch checked={inp.randomGeneratedIfEmpty} onCheckedChange={v => update(i, { randomGeneratedIfEmpty: v })} />
                        </div>
                    </Row>
                    <div className="flex justify-end">
                        <Button size="sm" variant="ghost" onClick={() => onChange(inputs.filter((_, idx) => idx !== i))}>
                            <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                    </div>
                </div>
            ))}
        </div>
    );
}
