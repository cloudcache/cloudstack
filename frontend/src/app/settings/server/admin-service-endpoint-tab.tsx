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
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Plus, Pencil, Trash } from "lucide-react";

// A shared admin tab for the three service-endpoint kinds (MQ / SMTP / Redis).
// Each kind ships a different `fields` definition; everything else is reused.

export interface FieldDef {
    key: string;
    label: string;
    type: 'text' | 'number' | 'password' | 'switch';
    placeholder?: string;
    defaultValue?: any;
    /** When omitted in edit mode, leaves the existing value unchanged. Used for passwords. */
    optionalOnEdit?: boolean;
}

export interface ListColumn {
    /** Row → string for badge/inline display */
    render: (item: any) => React.ReactNode;
}

interface Props {
    title: string;
    description: string;
    items: any[];
    fields: FieldDef[];
    /** Inline columns shown next to the name */
    columns?: ListColumn[];
    createFn: (body: any) => Promise<any>;
    updateFn: (id: string, body: any) => Promise<any>;
    deleteFn: (id: string) => Promise<any>;
    emptyLabel?: string;
}

function buildInitial(fields: FieldDef[], existing?: any): Record<string, any> {
    const out: Record<string, any> = {};
    for (const f of fields) {
        if (existing && existing[f.key] !== undefined && existing[f.key] !== null) {
            out[f.key] = existing[f.key];
        } else if (f.defaultValue !== undefined) {
            out[f.key] = f.defaultValue;
        } else if (f.type === 'switch') {
            out[f.key] = false;
        } else if (f.type === 'number') {
            out[f.key] = 0;
        } else {
            out[f.key] = '';
        }
    }
    return out;
}

export default function AdminServiceEndpointTab({
    title, description, items: initial, fields, columns,
    createFn, updateFn, deleteFn, emptyLabel,
}: Props) {
    const [items, setItems] = useState<any[]>(initial);
    const [showCreate, setShowCreate] = useState(false);
    const [editItem, setEditItem] = useState<any | null>(null);
    const [form, setForm] = useState<Record<string, any>>(() => buildInitial(fields));
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const startCreate = () => {
        setForm(buildInitial(fields));
        setShowCreate(true);
    };
    const startEdit = (it: any) => {
        // Reset passwords to empty so we don't overwrite-stripe accidentally
        const stripped = { ...it };
        for (const f of fields) {
            if (f.optionalOnEdit) stripped[f.key] = '';
        }
        setForm(buildInitial(fields, stripped));
        setEditItem(it);
    };

    const submitCreate = async () => {
        setSaving(true);
        const result = await createFn(form);
        setSaving(false);
        if (result?.status === 'success') {
            setShowCreate(false);
            window.location.reload();
        }
    };
    const submitEdit = async () => {
        if (!editItem) return;
        setSaving(true);
        const body: any = {};
        for (const f of fields) {
            const v = form[f.key];
            if (f.optionalOnEdit && (v === '' || v == null)) continue;
            body[f.key] = v;
        }
        const result = await updateFn(editItem.id, body);
        setSaving(false);
        if (result?.status === 'success') {
            setItems(prev => prev.map(i => i.id === editItem.id ? { ...i, ...body } : i));
            setEditItem(null);
        }
    };
    const handleDelete = async (it: any) => {
        const confirmed = await openConfirmDialog({
            title: `Delete ${it.name}?`,
            description: `Removes ${it.name} permanently.`,
            okButton: 'Delete', cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteFn(it.id));
        setItems(prev => prev.filter(i => i.id !== it.id));
    };

    const renderField = (f: FieldDef) => {
        if (f.type === 'switch') {
            return (
                <div className="flex items-center justify-between">
                    <Label>{f.label}</Label>
                    <Switch
                        checked={!!form[f.key]}
                        onCheckedChange={(v) => setForm(s => ({ ...s, [f.key]: v }))} />
                </div>
            );
        }
        return (
            <div className="space-y-2">
                <Label>{f.label}{f.optionalOnEdit && editItem ? ' (leave blank to keep current)' : ''}</Label>
                <Input
                    type={f.type === 'password' ? 'password' : f.type === 'number' ? 'number' : 'text'}
                    placeholder={f.placeholder}
                    value={form[f.key] ?? ''}
                    onChange={(e) => {
                        const raw = e.target.value;
                        const v = f.type === 'number' ? (raw === '' ? '' : Number(raw)) : raw;
                        setForm(s => ({ ...s, [f.key]: v }));
                    }} />
            </div>
        );
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{title}</CardTitle>
                        <CardDescription>{description}</CardDescription>
                    </div>
                    <Button size="sm" onClick={startCreate}><Plus className="mr-2 h-4 w-4" />Add</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{emptyLabel ?? 'No endpoints configured yet.'}</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((it) => (
                                <div key={it.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="space-y-0.5">
                                        <div className="flex items-center gap-2 flex-wrap">
                                            <span className="font-medium">{it.name}</span>
                                            {columns?.map((c, idx) => <span key={idx}>{c.render(it)}</span>)}
                                            <Badge variant={it.is_active !== false ? 'default' : 'secondary'}>
                                                {it.is_active !== false ? 'active' : 'inactive'}
                                            </Badge>
                                        </div>
                                        {it.description && <div className="text-xs text-muted-foreground">{it.description}</div>}
                                    </div>
                                    <div className="flex items-center gap-1">
                                        <Button size="sm" variant="ghost" onClick={() => startEdit(it)}>
                                            <Pencil className="h-4 w-4" />
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => handleDelete(it)}>
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
                        <DialogTitle>Add {title.replace(/s$/, '')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">{fields.map(f => <div key={f.key}>{renderField(f)}</div>)}</div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowCreate(false)}>Cancel</Button>
                        <Button onClick={submitCreate} disabled={saving}>{saving ? 'Saving…' : 'Create'}</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            {/* Edit dialog */}
            <Dialog open={!!editItem} onOpenChange={(v) => !v && setEditItem(null)}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{editItem?.name}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">{fields.map(f => <div key={f.key}>{renderField(f)}</div>)}</div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setEditItem(null)}>Cancel</Button>
                        <Button onClick={submitEdit} disabled={saving}>{saving ? 'Saving…' : 'Save'}</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
