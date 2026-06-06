'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { addNode, deleteNode, setNodeSchedulable, updateNode, reprovisionNode } from "./actions";
import { Plus, Trash, Pencil, RotateCcw, AlertTriangle } from "lucide-react";
import { useT } from "@/i18n";

interface NodeItem {
    id: string;
    hostname: string;
    ip_address: string;
    node_role?: string;
    node_status: string;
    provision_error?: string;
    cpu_capacity_mcores?: number;
    mem_capacity_mb?: number;
    storage_path?: string;
    cluster_name?: string;
    cluster_display_name?: string;
    cluster_id?: string;
    cluster_orchestrator?: 'K3S' | 'DOCKER';
    pool_name?: string;
    pool_display_name?: string;
    ip_pool_name?: string;
    ip_pool_cidr?: string;
    ip_pool_gateway?: string;
    last_seen_at?: string;
}

interface ClusterOption {
    id: string;
    name: string;
    display_name?: string;
    orchestrator?: 'K3S' | 'DOCKER';
}

export default function AdminNodesTab({ initialItems, clusters }: { initialItems: NodeItem[]; clusters: ClusterOption[] }) {
    const t = useT();
    const [items, setItems] = useState<NodeItem[]>(initialItems);
    const [showAdd, setShowAdd] = useState(false);
    const [form, setForm] = useState({ cluster_id: '', hostname: '', ip_address: '', ssh_password: '', node_role: 'MASTER', storage_path: '/storage' });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    // Edit state
    const [editNode, setEditNode] = useState<NodeItem | null>(null);
    const [editForm, setEditForm] = useState({ hostname: '', ip_address: '', node_role: '', storage_path: '' });
    const [editSaving, setEditSaving] = useState(false);

    // Reprovision state
    const [reprovNode, setReprovNode] = useState<NodeItem | null>(null);
    const [reprovPassword, setReprovPassword] = useState('');
    const [reprovSaving, setReprovSaving] = useState(false);

    const handleAdd = async () => {
        setSaving(true);
        const selectedCluster = clusters.find(c => c.id === form.cluster_id);
        const isDocker = selectedCluster?.orchestrator === 'DOCKER';
        const result = await addNode(null, {
            cluster_id: form.cluster_id,
            hostname: form.hostname,
            ip_address: form.ip_address,
            ssh_password: form.ssh_password,
            node_role: isDocker ? 'WORKER' : (form.node_role || undefined),
            storage_path: form.storage_path || undefined,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowAdd(false);
            setForm({ cluster_id: '', hostname: '', ip_address: '', ssh_password: '', node_role: 'MASTER', storage_path: '/storage' });
            window.location.reload();
        }
    };

    const handleEdit = (node: NodeItem) => {
        setEditForm({
            hostname: node.hostname,
            ip_address: node.ip_address,
            node_role: node.node_role || 'WORKER',
            storage_path: node.storage_path || '/storage',
        });
        setEditNode(node);
    };

    const handleEditSave = async () => {
        if (!editNode) return;
        setEditSaving(true);
        const result = await updateNode(editNode.id, {
            hostname: editForm.hostname,
            ip_address: editForm.ip_address,
            node_role: editForm.node_role,
            storage_path: editForm.storage_path,
        });
        setEditSaving(false);
        if (result?.status === 'success') {
            setEditNode(null);
            window.location.reload();
        }
    };

    const handleReprovision = (node: NodeItem) => {
        setReprovPassword('');
        setReprovNode(node);
    };

    const handleReprovisionSubmit = async () => {
        if (!reprovNode) return;
        setReprovSaving(true);
        const result = await reprovisionNode(reprovNode.id, reprovPassword);
        setReprovSaving(false);
        if (result?.status === 'success') {
            setReprovNode(null);
            // Update local state to show PROVISIONING
            setItems(prev => prev.map(n => n.id === reprovNode.id ? { ...n, node_status: 'PROVISIONING', provision_error: undefined } : n));
        }
    };

    const handleCordon = async (node: NodeItem, schedulable: boolean) => {
        const confirmed = await openConfirmDialog({
            title: schedulable ? t('admin.nodes.activateNode') : t('admin.nodes.cordonNode'),
            description: schedulable
                ? t('admin.nodes.activateDescription', { hostname: node.hostname })
                : t('admin.nodes.cordonDescription', { hostname: node.hostname }),
            okButton: schedulable ? t('admin.nodes.activate') : t('admin.nodes.cordon'),
            cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        Toast.fromAction(() => setNodeSchedulable(node.id, schedulable));
    };

    const handleDelete = async (node: NodeItem) => {
        const confirmed = await openConfirmDialog({
            title: t('admin.nodes.deleteNode'),
            description: t('admin.nodes.deleteDescription', { hostname: node.hostname, ip: node.ip_address }),
            okButton: t('common.delete'),
            cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteNode(node.id));
        setItems(prev => prev.filter(i => i.id !== node.id));
    };

    const isActive = (node: NodeItem) => node.node_status === 'ACTIVE' || node.node_status === 'READY' || node.node_status === 'ready';
    const isFailed = (node: NodeItem) => node.node_status === 'NOT_READY' || node.node_status === 'UNKNOWN';

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{t('admin.nodes.title')}</CardTitle>
                        <CardDescription>{t('admin.nodes.description')}</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowAdd(true)}><Plus className="mr-2 h-4 w-4" />{t('admin.nodes.addNode')}</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t('admin.nodes.empty')}</p>
                    ) : (
                        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                            {items.map((node) => (
                                <div key={node.id} className="border rounded-lg p-4 space-y-2">
                                    <div className="flex items-center justify-between">
                                        <span className="font-semibold truncate">{node.hostname}</span>
                                        <Badge variant={isActive(node) ? 'default' : node.node_status === 'PROVISIONING' ? 'secondary' : 'destructive'}>
                                            {node.node_status}
                                        </Badge>
                                    </div>
                                    <div className="text-sm text-muted-foreground space-y-1">
                                        <div><span className="font-medium">{t('admin.nodes.ip')}:</span> {node.ip_address}</div>
                                        {node.node_role && <div><span className="font-medium">{t('admin.nodes.role')}:</span> {node.node_role}</div>}
                                        {node.cluster_name && (
                                            <div className="flex items-center gap-1 flex-wrap">
                                                <span className="font-medium">{t('admin.nodes.cluster')}:</span>
                                                <span>{node.cluster_display_name ?? node.cluster_name}</span>
                                                {node.cluster_display_name && node.cluster_name && node.cluster_display_name !== node.cluster_name && (
                                                    <span className="text-xs text-muted-foreground">({node.cluster_name})</span>
                                                )}
                                                {node.cluster_orchestrator && (
                                                    <Badge variant="outline" className="ml-1">{node.cluster_orchestrator}</Badge>
                                                )}
                                            </div>
                                        )}
                                        {(node.pool_display_name || node.pool_name) && (
                                            <div>
                                                <span className="font-medium">{t('admin.nodes.pool')}:</span> {node.pool_display_name ?? node.pool_name}
                                                {node.pool_display_name && node.pool_name && node.pool_display_name !== node.pool_name && (
                                                    <span className="text-xs text-muted-foreground"> ({node.pool_name})</span>
                                                )}
                                            </div>
                                        )}
                                        {node.ip_pool_name && (
                                            <div>
                                                <span className="font-medium">IP pool:</span>{' '}
                                                <span className="font-mono text-xs">
                                                    {node.ip_pool_name} · {node.ip_pool_cidr}
                                                    {node.ip_pool_gateway ? ` gw ${node.ip_pool_gateway}` : ''}
                                                </span>
                                            </div>
                                        )}
                                        {node.cpu_capacity_mcores !== undefined && (
                                            <div><span className="font-medium">{t('admin.nodes.cpu')}:</span> {t('admin.nodes.cores', { value: (node.cpu_capacity_mcores / 1000).toFixed(1) })}</div>
                                        )}
                                        {node.mem_capacity_mb !== undefined && (
                                            <div><span className="font-medium">{t('admin.nodes.memory')}:</span> {t('admin.nodes.gb', { value: (node.mem_capacity_mb / 1024).toFixed(1) })}</div>
                                        )}
                                        {node.storage_path && <div><span className="font-medium">{t('admin.nodes.storage')}:</span> {node.storage_path}</div>}
                                        {node.last_seen_at && (
                                            <div><span className="font-medium">{t('admin.nodes.lastSeen')}:</span> {new Date(node.last_seen_at).toLocaleString()}</div>
                                        )}
                                    </div>

                                    {/* Provision error banner */}
                                    {node.provision_error && (
                                        <div className="flex items-start gap-2 rounded-md bg-destructive/10 p-2 text-xs text-destructive">
                                            <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                                            <span className="break-all">{node.provision_error}</span>
                                        </div>
                                    )}

                                    <div className="flex flex-wrap gap-2 pt-2">
                                        {/* Edit button — always available except during provisioning */}
                                        {node.node_status !== 'PROVISIONING' && (
                                            <Button size="sm" variant="outline" onClick={() => handleEdit(node)}>
                                                <Pencil className="mr-1 h-3 w-3" /> {t('common.edit')}
                                            </Button>
                                        )}

                                        {/* Retry button — only for failed nodes */}
                                        {isFailed(node) && (
                                            <Button size="sm" variant="outline" onClick={() => handleReprovision(node)}>
                                                <RotateCcw className="mr-1 h-3 w-3" /> {t('common.retry')}
                                            </Button>
                                        )}

                                        {/* Cordon/Uncordon — only for active or not-ready (not provisioning) */}
                                        {isActive(node) ? (
                                            <Button size="sm" variant="outline" onClick={() => handleCordon(node, false)}>
                                                {t('admin.nodes.cordon')}
                                            </Button>
                                        ) : node.node_status !== 'PROVISIONING' && !isFailed(node) ? (
                                            <Button size="sm" variant="outline" onClick={() => handleCordon(node, true)}>
                                                {t('admin.nodes.uncordon')}
                                            </Button>
                                        ) : null}

                                        <Button size="sm" variant="ghost" onClick={() => handleDelete(node)}>
                                            <Trash className="h-4 w-4 text-destructive" />
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            {/* Add Node Dialog */}
            <Dialog open={showAdd} onOpenChange={setShowAdd}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('admin.nodes.addNode')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t('admin.nodes.cluster')}</Label>
                            <Select value={form.cluster_id} onValueChange={v => setForm(f => ({ ...f, cluster_id: v }))}>
                                <SelectTrigger>
                                    <SelectValue placeholder={t('admin.nodes.selectCluster')} />
                                </SelectTrigger>
                                <SelectContent>
                                    {clusters.map(c => (
                                        <SelectItem key={c.id} value={c.id}>{c.display_name ?? c.name}</SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </div>
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.hostname')}</Label>
                                <Input value={form.hostname} onChange={e => setForm(f => ({ ...f, hostname: e.target.value }))} placeholder="node-01" />
                            </div>
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.ipAddress')}</Label>
                                <Input value={form.ip_address} onChange={e => setForm(f => ({ ...f, ip_address: e.target.value }))} placeholder="10.0.0.10" />
                            </div>
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.nodes.sshPassword')}</Label>
                            <Input type="password" value={form.ssh_password} onChange={e => setForm(f => ({ ...f, ssh_password: e.target.value }))} placeholder={t('admin.nodes.sshPasswordPlaceholder')} />
                        </div>
                        {(() => {
                            const selectedCluster = clusters.find(c => c.id === form.cluster_id);
                            const isDocker = selectedCluster?.orchestrator === 'DOCKER';
                            return (
                                <>
                                    <div className="grid grid-cols-2 gap-4">
                                        {!isDocker && (
                                            <div className="space-y-2">
                                                <Label>{t('admin.nodes.role')}</Label>
                                                <Select value={form.node_role} onValueChange={v => setForm(f => ({ ...f, node_role: v }))}>
                                                    <SelectTrigger>
                                                        <SelectValue />
                                                    </SelectTrigger>
                                                    <SelectContent>
                                                        <SelectItem value="MASTER">{t('admin.nodes.master')}</SelectItem>
                                                        <SelectItem value="WORKER">{t('admin.nodes.worker')}</SelectItem>
                                                    </SelectContent>
                                                </Select>
                                            </div>
                                        )}
                                        <div className="space-y-2">
                                            <Label>{t('admin.nodes.storagePath')}</Label>
                                            <Input value={form.storage_path} onChange={e => setForm(f => ({ ...f, storage_path: e.target.value }))} placeholder="/storage" />
                                        </div>
                                    </div>
                                    <p className="text-xs text-muted-foreground">
                                        {isDocker
                                            ? 'Docker cluster: all nodes run docker + qs-agent as workers.'
                                            : <>{t('admin.nodes.masterFirstPrefix')} <strong>{t('admin.nodes.master')}</strong> {t('admin.nodes.masterFirstSuffix')}</>}
                                    </p>
                                </>
                            );
                        })()}
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowAdd(false)}>{t('common.cancel')}</Button>
                        <Button onClick={handleAdd} disabled={saving || !form.cluster_id || !form.hostname || !form.ip_address || !form.ssh_password}>
                            {saving ? t('admin.nodes.adding') : t('admin.nodes.addNode')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Edit Node Dialog */}
            <Dialog open={!!editNode} onOpenChange={open => { if (!open) setEditNode(null); }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('admin.nodes.editNode')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.hostname')}</Label>
                                <Input value={editForm.hostname} onChange={e => setEditForm(f => ({ ...f, hostname: e.target.value }))} />
                            </div>
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.ipAddress')}</Label>
                                <Input value={editForm.ip_address} onChange={e => setEditForm(f => ({ ...f, ip_address: e.target.value }))} />
                            </div>
                        </div>
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.role')}</Label>
                                <Select value={editForm.node_role} onValueChange={v => setEditForm(f => ({ ...f, node_role: v }))}>
                                    <SelectTrigger>
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="MASTER">{t('admin.nodes.master')}</SelectItem>
                                        <SelectItem value="WORKER">{t('admin.nodes.worker')}</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>
                            <div className="space-y-2">
                                <Label>{t('admin.nodes.storagePath')}</Label>
                                <Input value={editForm.storage_path} onChange={e => setEditForm(f => ({ ...f, storage_path: e.target.value }))} />
                            </div>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setEditNode(null)}>{t('common.cancel')}</Button>
                        <Button onClick={handleEditSave} disabled={editSaving || !editForm.hostname || !editForm.ip_address}>
                            {editSaving ? t('common.saving') : t('common.save')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Reprovision Dialog */}
            <Dialog open={!!reprovNode} onOpenChange={open => { if (!open) setReprovNode(null); }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('admin.nodes.retryProvisioning')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <p className="text-sm text-muted-foreground">
                            {t('admin.nodes.retryProvisioningFor')} <strong>{reprovNode?.hostname}</strong> ({reprovNode?.ip_address}).
                            {t('admin.nodes.retryProvisioningDescription')}
                        </p>
                        {reprovNode?.provision_error && (
                            <div className="flex items-start gap-2 rounded-md bg-destructive/10 p-3 text-sm text-destructive">
                                <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                                <div>
                                    <div className="font-medium mb-1">{t('admin.nodes.previousError')}:</div>
                                    <span className="break-all">{reprovNode.provision_error}</span>
                                </div>
                            </div>
                        )}
                        <div className="space-y-2">
                            <Label>{t('admin.nodes.sshPassword')}</Label>
                            <Input type="password" value={reprovPassword} onChange={e => setReprovPassword(e.target.value)} placeholder={t('admin.nodes.rootSshPassword')} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setReprovNode(null)}>{t('common.cancel')}</Button>
                        <Button onClick={handleReprovisionSubmit} disabled={reprovSaving || !reprovPassword}>
                            {reprovSaving ? t('admin.nodes.starting') : t('admin.nodes.retryProvisioning')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
