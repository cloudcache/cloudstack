'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { createResourcePool, deleteResourcePool } from "./actions";
import { Plus, Trash } from "lucide-react";

interface ResourcePool {
    id: string;
    name: string;
    display_name: string;
    region?: string;
    description?: string;
    is_active?: boolean;
}

export default function AdminResourcePoolsTab({ initialItems }: { initialItems: ResourcePool[] }) {
    const [items, setItems] = useState<ResourcePool[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({ name: '', display_name: '', region: '', description: '' });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

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

    const handleDelete = async (item: ResourcePool) => {
        const confirmed = await openConfirmDialog({
            title: 'Delete Resource Pool',
            description: `Delete resource pool "${item.display_name}"? All clusters in this pool will also be affected.`,
            okButton: 'Delete',
            cancelButton: 'Cancel',
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
                        <CardTitle>Resource Pools</CardTitle>
                        <CardDescription>Logical groupings of compute clusters (e.g. by region or datacenter).</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No resource pools configured yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{item.display_name}</span>
                                            <span className="text-muted-foreground text-xs">({item.name})</span>
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? 'Active' : 'Inactive'}
                                            </Badge>
                                        </div>
                                        {item.region && <div className="text-sm text-muted-foreground">Region: {item.region}</div>}
                                        {item.description && <div className="text-xs text-muted-foreground">{item.description}</div>}
                                    </div>
                                    <Button size="sm" variant="ghost" onClick={() => handleDelete(item)}>
                                        <Trash className="h-4 w-4 text-destructive" />
                                    </Button>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            <Dialog open={showCreate} onOpenChange={setShowCreate}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Add Resource Pool</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Identifier (slug)</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="us-east-1" />
                        </div>
                        <div className="space-y-2">
                            <Label>Display Name</Label>
                            <Input value={form.display_name} onChange={e => setForm(f => ({ ...f, display_name: e.target.value }))} placeholder="US East" />
                        </div>
                        <div className="space-y-2">
                            <Label>Region (optional)</Label>
                            <Input value={form.region} onChange={e => setForm(f => ({ ...f, region: e.target.value }))} placeholder="us-east-1" />
                        </div>
                        <div className="space-y-2">
                            <Label>Description (optional)</Label>
                            <Input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreate(false)}>Cancel</Button>
                        <Button onClick={handleCreate} disabled={saving}>{saving ? 'Saving…' : 'Create'}</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
