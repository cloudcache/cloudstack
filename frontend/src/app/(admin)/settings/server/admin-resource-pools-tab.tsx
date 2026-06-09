'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Toast } from "@/frontend/utils/toast.utils";
import { toast } from "sonner";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { createResourcePool, updateResourcePool, deleteResourcePool } from "./actions";
import { Plus, Trash, Pencil } from "lucide-react";
import { useT } from "@/i18n";

interface ResourcePool {
    id: string;
    name: string;
    display_name: string;
    region?: string;
    description?: string;
    is_active?: boolean;
    cluster_count?: number;
}

export default function AdminResourcePoolsTab({ initialItems }: { initialItems: ResourcePool[] }) {
    const t = useT();
    const [items, setItems] = useState<ResourcePool[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({ name: '', display_name: '', region: '', description: '' });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    // Edit state
    const [editItem, setEditItem] = useState<ResourcePool | null>(null);
    const [editForm, setEditForm] = useState({ display_name: '', region: '', description: '', is_active: true });
    const [editSaving, setEditSaving] = useState(false);

    const handleCreate = async () => {
        setSaving(true);
        const result = await createResourcePool(null, {
            name: form.name,
            display_name: form.display_name,
            region: form.region || undefined,
            description: form.description || undefined,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ name: '', display_name: '', region: '', description: '' });
            window.location.reload();
        }
    };

    const handleEdit = (item: ResourcePool) => {
        setEditForm({
            display_name: item.display_name,
            region: item.region ?? '',
            description: item.description ?? '',
            is_active: item.is_active !== false,
        });
        setEditItem(item);
    };

    const handleEditSave = async () => {
        if (!editItem) return;
        setEditSaving(true);
        const result = await updateResourcePool(editItem.id, {
            display_name: editForm.display_name || undefined,
            region: editForm.region || undefined,
            description: editForm.description || undefined,
            is_active: editForm.is_active,
        });
        setEditSaving(false);
        if (result?.status === 'success') {
            setItems(prev => prev.map(p => p.id === editItem.id ? {
                ...p,
                display_name: editForm.display_name,
                region: editForm.region || undefined,
                description: editForm.description || undefined,
                is_active: editForm.is_active,
            } : p));
            setEditItem(null);
        }
    };

    const handleDelete = async (item: ResourcePool) => {
        if ((item.cluster_count ?? 0) > 0) {
            toast.error(t('admin.resourcePools.hasClusters', { count: item.cluster_count ?? 0 }));
            return;
        }
        const confirmed = await openConfirmDialog({
            title: t('admin.resourcePools.deleteTitle'),
            description: t('admin.resourcePools.deleteDescription', { name: item.display_name }),
            okButton: t('common.delete'),
            cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteResourcePool(item.id));
        setItems(prev => prev.filter(i => i.id !== item.id));
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{t('admin.resourcePools.title')}</CardTitle>
                        <CardDescription>{t('admin.resourcePools.description')}</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />{t('common.add')}</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t('admin.resourcePools.empty')}</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2 flex-wrap">
                                            <span className="font-medium">{item.display_name}</span>
                                            <span className="text-muted-foreground text-xs">({item.name})</span>
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? t('plans.active') : t('plans.inactive')}
                                            </Badge>
                                            <Badge variant="secondary">
                                                {item.cluster_count ?? 0} {t('admin.resourcePools.clusters')}
                                            </Badge>
                                        </div>
                                        {item.region && <div className="text-sm text-muted-foreground">{t('admin.resourcePools.region')}: {item.region}</div>}
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

            {/* Create dialog */}
            <Dialog open={showCreate} onOpenChange={setShowCreate}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t('admin.resourcePools.addTitle')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t('admin.resourcePools.identifierSlug')}</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="us-east-1" />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('plans.displayName')}</Label>
                            <Input value={form.display_name} onChange={e => setForm(f => ({ ...f, display_name: e.target.value }))} placeholder="US East" />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.resourcePools.regionOptional')}</Label>
                            <Input value={form.region} onChange={e => setForm(f => ({ ...f, region: e.target.value }))} placeholder="us-east-1" />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.common.descriptionOptional')}</Label>
                            <Input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreate(false)}>{t('common.cancel')}</Button>
                        <Button onClick={handleCreate} disabled={saving || !form.name || !form.display_name}>
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
                            <Label>{t('plans.displayName')}</Label>
                            <Input value={editForm.display_name} onChange={e => setEditForm(f => ({ ...f, display_name: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.resourcePools.regionOptional')}</Label>
                            <Input value={editForm.region} onChange={e => setEditForm(f => ({ ...f, region: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.common.descriptionOptional')}</Label>
                            <Input value={editForm.description} onChange={e => setEditForm(f => ({ ...f, description: e.target.value }))} />
                        </div>
                        <div className="flex items-center justify-between">
                            <Label>{t('plans.active')}</Label>
                            <Switch checked={editForm.is_active} onCheckedChange={v => setEditForm(f => ({ ...f, is_active: v }))} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setEditItem(null)}>{t('common.cancel')}</Button>
                        <Button onClick={handleEditSave} disabled={editSaving || !editForm.display_name}>
                            {editSaving ? t('common.saving') : t('common.save')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
