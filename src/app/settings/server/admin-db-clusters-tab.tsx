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
import { createDbCluster, deleteDbCluster } from "./actions";
import { Plus, Trash } from "lucide-react";

interface DbCluster {
    id: string;
    name: string;
    cluster_type: string;
    host: string;
    port: number;
    admin_user: string;
    max_databases?: number;
    description?: string;
    is_active?: boolean;
}

const DB_TYPES = ['mysql', 'postgres', 'mariadb', 'mongodb'];

export default function AdminDbClustersTab({ initialItems }: { initialItems: DbCluster[] }) {
    const [items, setItems] = useState<DbCluster[]>(initialItems);
    const [showCreate, setShowCreate] = useState(false);
    const [form, setForm] = useState({
        name: '', cluster_type: 'mysql', host: '', port: '3306',
        admin_user: '', admin_password: '', max_databases: '', description: '', manager_url: '',
    });
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const handleCreate = async () => {
        setSaving(true);
        const result = await createDbCluster(null, {
            name: form.name,
            cluster_type: form.cluster_type,
            host: form.host,
            port: parseInt(form.port, 10),
            admin_user: form.admin_user,
            admin_password: form.admin_password,
            max_databases: form.max_databases ? parseInt(form.max_databases, 10) : undefined,
            description: form.description || undefined,
            manager_url: form.manager_url || undefined,
        });
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            setForm({ name: '', cluster_type: 'mysql', host: '', port: '3306', admin_user: '', admin_password: '', max_databases: '', description: '', manager_url: '' });
            window.location.reload();
        }
    };

    const handleDelete = async (item: DbCluster) => {
        const confirmed = await openConfirmDialog({
            title: 'Delete DB Cluster',
            description: `Delete DB cluster "${item.name}"? This cannot be undone.`,
            okButton: 'Delete',
            cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteDbCluster(item.id));
        setItems(prev => prev.filter(i => i.id !== item.id));
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>DB Clusters</CardTitle>
                        <CardDescription>External database clusters where user-provisioned databases are created.</CardDescription>
                    </div>
                    <Button size="sm" onClick={() => setShowCreate(true)}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No DB clusters configured yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((item) => (
                                <div key={item.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{item.name}</span>
                                            <Badge variant="outline">{item.cluster_type}</Badge>
                                            <Badge variant={item.is_active !== false ? 'default' : 'secondary'}>
                                                {item.is_active !== false ? 'Active' : 'Inactive'}
                                            </Badge>
                                        </div>
                                        <div className="text-sm text-muted-foreground font-mono">{item.host}:{item.port}</div>
                                        <div className="text-xs text-muted-foreground">User: {item.admin_user}</div>
                                        {item.max_databases !== undefined && (
                                            <div className="text-xs text-muted-foreground">Max DBs: {item.max_databases}</div>
                                        )}
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
                <DialogContent className="max-w-md">
                    <DialogHeader>
                        <DialogTitle>Add DB Cluster</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Name</Label>
                            <Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="prod-mysql" />
                        </div>
                        <div className="space-y-2">
                            <Label>DB Type</Label>
                            <Select value={form.cluster_type} onValueChange={v => setForm(f => ({ ...f, cluster_type: v, port: v === 'postgres' ? '5432' : v === 'mongodb' ? '27017' : '3306' }))}>
                                <SelectTrigger><SelectValue /></SelectTrigger>
                                <SelectContent>
                                    {DB_TYPES.map(t => <SelectItem key={t} value={t}>{t}</SelectItem>)}
                                </SelectContent>
                            </Select>
                        </div>
                        <div className="grid grid-cols-3 gap-2">
                            <div className="col-span-2 space-y-2">
                                <Label>Host</Label>
                                <Input value={form.host} onChange={e => setForm(f => ({ ...f, host: e.target.value }))} placeholder="db.example.com" className="font-mono" />
                            </div>
                            <div className="space-y-2">
                                <Label>Port</Label>
                                <Input value={form.port} onChange={e => setForm(f => ({ ...f, port: e.target.value }))} className="font-mono" />
                            </div>
                        </div>
                        <div className="space-y-2">
                            <Label>Admin User</Label>
                            <Input value={form.admin_user} onChange={e => setForm(f => ({ ...f, admin_user: e.target.value }))} placeholder="root" />
                        </div>
                        <div className="space-y-2">
                            <Label>Admin Password</Label>
                            <Input type="password" value={form.admin_password} onChange={e => setForm(f => ({ ...f, admin_password: e.target.value }))} />
                        </div>
                        <div className="space-y-2">
                            <Label>Max Databases (optional)</Label>
                            <Input value={form.max_databases} onChange={e => setForm(f => ({ ...f, max_databases: e.target.value }))} type="number" placeholder="100" />
                        </div>
                        <div className="space-y-2">
                            <Label>Manager URL (optional)</Label>
                            <Input value={form.manager_url} onChange={e => setForm(f => ({ ...f, manager_url: e.target.value }))} placeholder="http://dbgate.example.com" />
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
