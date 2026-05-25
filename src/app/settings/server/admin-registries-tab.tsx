'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Checkbox } from "@/components/ui/checkbox";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { createRegistry, deleteRegistry } from "./actions";
import { Plus, Trash } from "lucide-react";

interface Registry {
    id: string;
    name: string;
    endpoint: string;
    username?: string;
    is_default?: boolean;
    is_active?: boolean;
}

export default function AdminRegistriesTab({ initialItems }: { initialItems: Registry[] }) {
    const [items, setItems] = useState<Registry[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({ name: '', endpoint: '', username: '', password: '', is_default: false });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const handleCreate = async () => {
        setSaving(true);
        const result = await createRegistry(null, {
            ...form,
            username: form.username || undefined,
            password: form.password || undefined,
        } as any);
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ name: '', endpoint: '', username: '', password: '', is_default: false });
            window.location.reload();
        }
    };

    const handleDelete = async (item: Registry) => {
        const confirmed = await openConfirmDialog({
            title: 'Delete Registry',
            description: `Delete registry "${item.name}"? This cannot be undone.`,
            okButton: 'Delete',
            cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteRegistry(item.id));
        setItems(prev => prev.filter(i => i.id !== item.id));
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>Image Registries</CardTitle>
                        <CardDescription>Configure container image registries used for pulling and pushing images.</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No registries configured yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{item.name}</span>
                                            {item.is_default && <Badge>Default</Badge>}
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? 'Active' : 'Inactive'}
                                            </Badge>
                                        </div>
                                        <div className="text-sm text-muted-foreground">{item.endpoint}</div>
                                        {item.username && <div className="text-xs text-muted-foreground">User: {item.username}</div>}
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
                        <DialogTitle>Add Registry</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Name</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="my-registry" />
                        </div>
                        <div className="space-y-2">
                            <Label>Endpoint</Label>
                            <Input value={form.endpoint} onChange={e => setForm(f => ({ ...f, endpoint: e.target.value }))} placeholder="registry.example.com" />
                        </div>
                        <div className="space-y-2">
                            <Label>Username (optional)</Label>
                            <Input value={form.username} onChange={e => setForm(f => ({ ...f, username: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>Password (optional)</Label>
                            <Input type="password" value={form.password} onChange={e => setForm(f => ({ ...f, password: e.target.value }))} />
                        </div>
                        <div className="flex items-center gap-2">
                            <Checkbox
                                id="is_default"
                                checked={form.is_default}
                                onCheckedChange={v => setForm(f => ({ ...f, is_default: !!v }))}
                            />
                            <Label htmlFor="is_default">Set as default registry</Label>
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
