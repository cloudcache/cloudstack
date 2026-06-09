'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Toast } from "@/frontend/utils/toast.utils";
import { toast } from "sonner";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { createCluster, updateCluster, deleteCluster } from "./actions";
import { Plus, Trash, Pencil } from "lucide-react";
import { useT } from "@/i18n";

interface PoolRef {
    id: string;
    name: string;
    display_name?: string;
}

interface IpPoolRef {
    id: string;
    name: string;
    cidr: string;
    gateway?: string;
}

interface IpPoolSnapshot {
    name?: string;
    cidr?: string;
    gateway?: string;
}

interface Cluster {
    id: string;
    name: string;
    display_name?: string;
    is_active?: boolean;
    description?: string;
    orchestrator?: 'K3S' | 'DOCKER';
    pool?: PoolRef;
    ip_pool_id?: string;
    ip_pool?: IpPoolSnapshot;
    node_main_iface?: string;
    node_count?: number;
    ready_count?: number;
}

export default function AdminClustersTab({ initialItems, clusterStorage, pools, ipPools }: { initialItems: Cluster[]; clusterStorage: any; pools: PoolRef[]; ipPools: IpPoolRef[] }) {
    const t = useT();
    const [items, setItems] = useState<Cluster[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState<{ name: string; display_name: string; description: string; pool_id: string; ip_pool_id: string; orchestrator: 'K3S' | 'DOCKER' }>({
        name: '', display_name: '', description: '',
        pool_id: pools[0]?.id ?? '',
        ip_pool_id: '',
        orchestrator: 'K3S',
    });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    // Edit state
    const [editItem, setEditItem] = useState<Cluster | null>(null);
    const [editForm, setEditForm] = useState({ display_name: '', description: '', is_active: true, ip_pool_id: '', node_main_iface: '' });
    const [editSaving, setEditSaving] = useState(false);

    const handleCreate = async () => {
        if (!form.pool_id) {
            toast.error(t('admin.clusters.poolRequired'));
            return;
        }
        setSaving(true);
        const result = await createCluster(null, {
            name: form.name,
            display_name: form.display_name || undefined,
            description: form.description || undefined,
            pool_id: form.pool_id,
            ip_pool_id: form.ip_pool_id || undefined,
            orchestrator: form.orchestrator,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ name: '', display_name: '', description: '', pool_id: pools[0]?.id ?? '', ip_pool_id: '', orchestrator: 'K3S' });
            window.location.reload();
        }
    };

    const handleEdit = (item: Cluster) => {
        setEditForm({
            display_name: item.display_name ?? '',
            description: item.description ?? '',
            is_active: item.is_active !== false,
            ip_pool_id: item.ip_pool_id ?? '',
            node_main_iface: item.node_main_iface ?? 'eth0',
        });
        setEditItem(item);
    };

    const handleEditSave = async () => {
        if (!editItem) return;
        setEditSaving(true);
        const result = await updateCluster(editItem.id, {
            display_name: editForm.display_name || undefined,
            description: editForm.description || undefined,
            is_active: editForm.is_active,
            ip_pool_id: editForm.ip_pool_id || undefined,
            node_main_iface: editForm.node_main_iface || undefined,
        });
        setEditSaving(false);
        if (result?.status === 'success') {
            // Rebuild the nested ip_pool snapshot from the selected pool so the
            // row reflects the assignment immediately (the list badge checks
            // `ip_pool`, not `ip_pool_id`); clears it when no pool is selected.
            const selectedPool = editForm.ip_pool_id
                ? ipPools.find(p => p.id === editForm.ip_pool_id)
                : undefined;
            setItems(prev => prev.map(c => c.id === editItem.id ? {
                ...c,
                display_name: editForm.display_name || undefined,
                description: editForm.description || undefined,
                is_active: editForm.is_active,
                ip_pool_id: editForm.ip_pool_id || undefined,
                ip_pool: selectedPool
                    ? { name: selectedPool.name, cidr: selectedPool.cidr, gateway: selectedPool.gateway }
                    : undefined,
                node_main_iface: editForm.node_main_iface || undefined,
            } : c));
            setEditItem(null);
        }
    };

    const handleDelete = async (item: Cluster) => {
        const confirmed = await openConfirmDialog({
            title: t('admin.clusters.deleteTitle'),
            description: t('admin.clusters.deleteDescription', { name: item.display_name ?? item.name }),
            okButton: t('common.delete'),
            cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteCluster(item.id));
        setItems(prev => prev.filter(i => i.id !== item.id));
    };

    return (
        <div className="space-y-4">
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{t('admin.clusters.title')}</CardTitle>
                        <CardDescription>{t('admin.clusters.description')}</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)} disabled={pools.length === 0}>
                        <Plus className="mr-2 h-4 w-4" />{t('common.add')}
                    </Button>
                </CardHeader>
                <CardContent>
                    {pools.length === 0 && (
                        <p className="text-sm text-amber-600 mb-3">{t('admin.clusters.noPools')}</p>
                    )}
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t('admin.clusters.empty')}</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2 flex-wrap">
                                            <span className="font-medium">{item.display_name ?? item.name}</span>
                                            {item.display_name && <span className="text-muted-foreground text-xs">({item.name})</span>}
                                            <Badge variant="outline">{item.orchestrator ?? 'K3S'}</Badge>
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? t('plans.active') : t('plans.inactive')}
                                            </Badge>
                                            {item.pool && (
                                                <Badge variant="secondary">
                                                    {t('admin.clusters.pool')}: {item.pool.display_name ?? item.pool.name}
                                                </Badge>
                                            )}
                                            {item.ip_pool ? (
                                                <Badge variant="secondary" className="font-mono">
                                                    IP: {item.ip_pool.name} · {item.ip_pool.cidr}
                                                    {item.ip_pool.gateway ? ` gw ${item.ip_pool.gateway}` : ''}
                                                </Badge>
                                            ) : (
                                                <Badge variant="outline" className="text-amber-600">no IP pool</Badge>
                                            )}
                                            {typeof item.node_count === 'number' && (
                                                <span className="text-xs text-muted-foreground">
                                                    {item.ready_count ?? 0}/{item.node_count} {t('admin.clusters.nodesReady')}
                                                </span>
                                            )}
                                        </div>
                                        {item.description && <div className="text-xs text-muted-foreground">{item.description}</div>}
                                    </div>
                                    <div className="flex items-center gap-1">
                                        <Button size="sm" variant="ghost" onClick={() => handleEdit(item)}>
                                            <Pencil className="h-4 w-4" />
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => handleDelete(item)}>
                                            <Trash className="h-4 w-4 text-destructive" />
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            {clusterStorage && (
                <Card>
                    <CardHeader>
                        <CardTitle>{t('admin.clusters.storageTitle')}</CardTitle>
                        <CardDescription>{t('admin.clusters.storageDescription')}</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <pre className="text-xs bg-muted rounded p-3 overflow-auto max-h-48">
                            {JSON.stringify(clusterStorage, null, 2)}
                        </pre>
                    </CardContent>
                </Card>
            )}

            {/* Create dialog */}
            <Dialog open={showCreate} onOpenChange={setShowCreate}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('admin.clusters.addTitle')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.pool')}</Label>
                            <Select value={form.pool_id} onValueChange={v => setForm(f => ({ ...f, pool_id: v }))}>
                                <SelectTrigger>
                                    <SelectValue placeholder={t('admin.clusters.selectPool')} />
                                </SelectTrigger>
                                <SelectContent>
                                    {pools.map(p => (
                                        <SelectItem key={p.id} value={p.id}>{p.display_name ?? p.name}</SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>
                        <div className="space-y-2">
                            <Label>Orchestrator</Label>
                            <Select value={form.orchestrator} onValueChange={(v: 'K3S' | 'DOCKER') => setForm(f => ({ ...f, orchestrator: v }))}>
                                <SelectTrigger>
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="K3S">K3s (Kubernetes)</SelectItem>
                                    <SelectItem value="DOCKER">Docker</SelectItem>
                                </SelectContent>
                            </Select>
                            <p className="text-xs text-muted-foreground">
                                {form.orchestrator === 'DOCKER'
                                    ? 'Nodes will run Docker Engine + qs-agent. All nodes are workers.'
                                    : 'Nodes will run K3s (Kubernetes). First node must be MASTER.'}
                            </p>
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.nameSlug')}</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="main-cluster" />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.displayNameOptional')}</Label>
                            <Input value={form.display_name} onChange={e => setForm(f => ({ ...f, display_name: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.common.descriptionOptional')}</Label>
                            <Input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.ipPoolOptional')}</Label>
                            <Select value={form.ip_pool_id || '__none__'} onValueChange={v => setForm(f => ({ ...f, ip_pool_id: v === '__none__' ? '' : v }))}>
                                <SelectTrigger>
                                    <SelectValue placeholder={t('admin.clusters.selectIpPool')} />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="__none__">— {t('common.none')} —</SelectItem>
                                    {ipPools.map(p => (
                                        <SelectItem key={p.id} value={p.id}>
                                            {p.name} ({p.cidr}{p.gateway ? ` gw ${p.gateway}` : ''})
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                            <p className="text-xs text-muted-foreground">{t('admin.clusters.ipPoolHint')}</p>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreate(false)}>{t('common.cancel')}</Button>
                        <Button onClick={handleCreate} disabled={saving || !form.name || !form.pool_id}>
                            {saving ? t('common.saving') : t('common.create')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Edit dialog */}
            <Dialog open={!!editItem} onOpenChange={v => !v && setEditItem(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{editItem?.name}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.displayNameOptional')}</Label>
                            <Input value={editForm.display_name} onChange={e => setEditForm(f => ({ ...f, display_name: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.common.descriptionOptional')}</Label>
                            <Input value={editForm.description} onChange={e => setEditForm(f => ({ ...f, description: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.ipPool')}</Label>
                            <Select value={editForm.ip_pool_id} onValueChange={v => setEditForm(f => ({ ...f, ip_pool_id: v }))}>
                                <SelectTrigger>
                                    <SelectValue placeholder={t('admin.clusters.selectIpPool')} />
                                </SelectTrigger>
                                <SelectContent>
                                    {ipPools.map(p => (
                                        <SelectItem key={p.id} value={p.id}>
                                            {p.name} ({p.cidr}{p.gateway ? ` gw ${p.gateway}` : ''})
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                            <p className="text-xs text-muted-foreground">{t('admin.clusters.ipPoolHint')}</p>
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.clusters.mainIface')}</Label>
                            <Input value={editForm.node_main_iface} onChange={e => setEditForm(f => ({ ...f, node_main_iface: e.target.value }))} placeholder="eth0" />
                            <p className="text-xs text-muted-foreground">{t('admin.clusters.mainIfaceHint')}</p>
                        </div>
                        <div className="flex items-center justify-between">
                            <Label>{t('plans.active')}</Label>
                            <Switch checked={editForm.is_active} onCheckedChange={v => setEditForm(f => ({ ...f, is_active: v }))} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setEditItem(null)}>{t('common.cancel')}</Button>
                        <Button onClick={handleEditSave} disabled={editSaving}>
                            {editSaving ? t('common.saving') : t('common.save')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
