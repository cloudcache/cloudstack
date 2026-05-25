'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "sonner";
import { setPlatformConfig } from "./actions";
import { Edit, Plus } from "lucide-react";

interface ConfigEntry {
    key: string;
    value: string;
    description?: string;
}

export default function AdminPlatformConfigTab({ initialConfig }: { initialConfig: ConfigEntry[] }) {
    const [config, setConfig] = useState<ConfigEntry[]>(initialConfig);
    const [editing, setEditing] = useState<ConfigEntry | null>(null);
    const [adding, setAdding] = useState(false);
    const [form, setForm] = useState({ key: '', value: '' });
    const [saving, setSaving] = useState(false);

    const handleSave = async () => {
        if (!form.key.trim()) return;
        setSaving(true);
        const result = await setPlatformConfig(form.key.trim(), form.value.trim());
        setSaving(false);
        if (result?.status === 'success') {
            setConfig(prev => {
                const idx = prev.findIndex(c => c.key === form.key);
                if (idx >= 0) {
                    const updated = [...prev];
                    updated[idx] = { ...updated[idx], value: form.value };
                    return updated;
                }
                return [...prev, { key: form.key, value: form.value }];
            });
            setEditing(null);
            setAdding(false);
            setForm({ key: '', value: '' });
            toast.success('Config saved.');
        }
    };

    const openEdit = (entry: ConfigEntry) => {
        setEditing(entry);
        setForm({ key: entry.key, value: entry.value });
    };

    const openAdd = () => {
        setAdding(true);
        setForm({ key: '', value: '' });
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>Platform Config</CardTitle>
                        <CardDescription>System-wide key/value configuration managed by the Rust backend.</CardDescription>
                    </div>
                    <Button size="sm" onClick={openAdd}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {config.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No platform config entries yet.</p>
                    ) : (
                        <div className="space-y-2">
                            {config.map((entry) => (
                                <div key={entry.key} className="flex items-start justify-between border rounded-lg p-3 gap-4">
                                    <div className="space-y-0.5 flex-1 min-w-0">
                                        <div className="font-mono text-sm font-medium truncate">{entry.key}</div>
                                        <div className="text-sm text-muted-foreground break-all">{entry.value}</div>
                                        {entry.description && <div className="text-xs text-muted-foreground">{entry.description}</div>}
                                    </div>
                                    <Button size="sm" variant="ghost" onClick={() => openEdit(entry)}>
                                        <Edit className="h-4 w-4" />
                                    </Button>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            <Dialog open={!!(editing || adding)} onOpenChange={(open) => { if (!open) { setEditing(null); setAdding(false); } }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{editing ? 'Edit Config Entry' : 'Add Config Entry'}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>Key</Label>
                            <Input
                                value={form.key}
                                onChange={e => setForm(f => ({ ...f, key: e.target.value }))}
                                disabled={!!editing}
                                className="font-mono"
                                placeholder="MY_CONFIG_KEY"
                            />
                        </div>
                        <div className="space-y-2">
                            <Label>Value</Label>
                            <Input
                                value={form.value}
                                onChange={e => setForm(f => ({ ...f, value: e.target.value }))}
                                placeholder="value"
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => { setEditing(null); setAdding(false); }}>Cancel</Button>
                        <Button onClick={handleSave} disabled={saving}>{saving ? 'Saving…' : 'Save'}</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
