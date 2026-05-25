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
import { createCluster, deleteCluster } from "./actions";
import { Plus, Trash } from "lucide-react";

interface Cluster {
    id: string;
    name: string;
    display_name?: string;
    pool_id: string;
    is_active?: boolean;
    description?: string;
}

export default function AdminClustersTab({ initialItems, clusterStorage }: { initialItems: Cluster[]; clusterStorage: any }) {
    const [items, setItems] = useState<Cluster[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({ pool_id: '', name: '', display_name: '', description: '' });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const handleCreate = async () => {
        setSaving(true);
        const result = await createCluster(null, {
            pool_id: form.pool_id,
            name: form.name,
            display_name: form.display_name || undefined,
            description: form.description || undefined,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ pool_id: '', name: '', display_name: '', description: '' });
            window.location.reload();
        }
    };

    const handleDelete = async (item: Cluster) => {
        const confirmed = await openConfirmDialog({
            title: 'Delete Cluster',
            description: `Delete cluster "${item.display_name ?? item.name}"? This will remove all associated nodes.`,
            okButton: 'Delete',
            cancelButton: 'Cancel',
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
                        <CardTitle>Clusters</CardTitle>
                        <CardDescription>K3s clusters managed by the system.</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No clusters configured yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{item.display_name ?? item.name}</span>
                                            {item.display_name && <span className="text-muted-foreground text-xs">({item.name})</span>}
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? 'Active' : 'Inactive'}
                                            </Badge>
                                        </div>
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

            {clusterStorage && (
                <Card>
                    <CardHeader>
                        <CardTitle>Cluster Storage</CardTitle>
                        <CardDescription>Cluster-wide storage configuration (read-only here — edit via platform config).</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <pre className="text-xs bg-muted rounded p-3 overflow-auto max-h-48">
                            {JSON.stringify(clusterStorage, null, 2)}
                        </pre>
                    </CardContent>
                </Card>
            )}

            <Dialog open={showCreate} onOpenChange={setShowCreate}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Add Cluster</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Resource Pool ID</Label>
                            <Input value={form.pool_id} onChange={e => setForm(f => ({ ...f, pool_id: e.target.value }))} placeholder="pool-uuid" />
                        </div>
                        <div className="space-y-2">
                            <Label>Cluster Name (slug)</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="main-cluster" />
                        </div>
                        <div className="space-y-2">
                            <Label>Display Name (optional)</Label>
                            <Input value={form.display_name} onChange={e => setForm(f => ({ ...f, display_name: e.target.value }))} />
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
        </div>
    );
}
