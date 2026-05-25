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
import { createIpPool, deleteIpPool } from "./actions";
import { Plus, Trash } from "lucide-react";

interface IpPool {
    id: string;
    name: string;
    cidr: string;
    gateway?: string;
    pool_type?: string;
    description?: string;
    is_active?: boolean;
}

export default function AdminIpPoolsTab({ initialItems }: { initialItems: IpPool[] }) {
    const [items, setItems] = useState<IpPool[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({ name: '', cidr: '', gateway: '', pool_type: '', description: '' });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const handleCreate = async () => {
        setSaving(true);
        const result = await createIpPool(null, {
            name: form.name,
            cidr: form.cidr,
            gateway: form.gateway || undefined,
            pool_type: form.pool_type || undefined,
            description: form.description || undefined,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ name: '', cidr: '', gateway: '', pool_type: '', description: '' });
            window.location.reload();
        }
    };

    const handleDelete = async (item: IpPool) => {
        const confirmed = await openConfirmDialog({
            title: 'Delete IP Pool',
            description: `Delete IP pool "${item.name}" (${item.cidr})? This cannot be undone.`,
            okButton: 'Delete',
            cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteIpPool(item.id));
        setItems(prev => prev.filter(i => i.id !== item.id));
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>IP Pools</CardTitle>
                        <CardDescription>CIDR blocks used for allocating IP addresses to services and nodes.</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No IP pools configured yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{item.name}</span>
                                            <span className="font-mono text-sm text-muted-foreground">{item.cidr}</span>
                                            {item.pool_type && <Badge variant="outline">{item.pool_type}</Badge>}
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? 'Active' : 'Inactive'}
                                            </Badge>
                                        </div>
                                        {item.gateway && <div className="text-sm text-muted-foreground">Gateway: {item.gateway}</div>}
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
                        <DialogTitle>Add IP Pool</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Name</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="public-pool" />
                        </div>
                        <div className="space-y-2">
                            <Label>CIDR</Label>
                            <Input value={form.cidr} onChange={e => setForm(f => ({ ...f, cidr: e.target.value }))} placeholder="10.0.0.0/24" className="font-mono" />
                        </div>
                        <div className="space-y-2">
                            <Label>Gateway (optional)</Label>
                            <Input value={form.gateway} onChange={e => setForm(f => ({ ...f, gateway: e.target.value }))} placeholder="10.0.0.1" className="font-mono" />
                        </div>
                        <div className="space-y-2">
                            <Label>Pool Type (optional)</Label>
                            <Input value={form.pool_type} onChange={e => setForm(f => ({ ...f, pool_type: e.target.value }))} placeholder="public / vpc / lb" />
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
