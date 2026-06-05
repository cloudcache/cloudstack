'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "sonner";
import { setPlatformConfig } from "./actions";
import { Edit, Plus, Save } from "lucide-react";
import { useT } from "@/i18n";

interface ConfigEntry {
    key: string;
    value: string;
    description?: string;
}

// ── NodePort Config Card ─────────────────────────────────────────────────────

function NodePortConfigCard({ initialConfig }: { initialConfig: ConfigEntry[] }) {
    const t = useT();
    const getVal = (key: string, fallback: string) =>
        initialConfig.find(c => c.key === key)?.value ?? fallback;

    const [rangeStart, setRangeStart] = useState(getVal('nodeport_range_start', '30000'));
    const [rangeEnd, setRangeEnd] = useState(getVal('nodeport_range_end', '32767'));
    const [reserved, setReserved] = useState(getVal('nodeport_reserved', '30100'));
    const [saving, setSaving] = useState(false);

    const handleSave = async () => {
        const start = parseInt(rangeStart, 10);
        const end = parseInt(rangeEnd, 10);
        if (isNaN(start) || isNaN(end) || start < 1 || end > 65535 || start >= end) {
            toast.error(t('admin.platform.invalidNodePortRange'));
            return;
        }
        // Validate reserved ports
        const reservedParts = reserved.split(',').map(s => s.trim()).filter(Boolean);
        for (const p of reservedParts) {
            const n = parseInt(p, 10);
            if (isNaN(n) || n < start || n > end) {
                toast.error(t('admin.platform.invalidReservedPort', { port: p, start, end }));
                return;
            }
        }

        setSaving(true);
        try {
            await setPlatformConfig('nodeport_range_start', String(start));
            await setPlatformConfig('nodeport_range_end', String(end));
            await setPlatformConfig('nodeport_reserved', reservedParts.join(','));
            toast.success(t('admin.platform.nodePortSaved'));
        } catch {
            toast.error(t('admin.platform.nodePortSaveFailed'));
        } finally {
            setSaving(false);
        }
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle className="text-base">{t('admin.platform.nodePortRange')}</CardTitle>
                <CardDescription>
                    {t('admin.platform.nodePortDescription')}
                </CardDescription>
            </CardHeader>
            <CardContent>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                    <div className="space-y-2">
                        <Label htmlFor="np-start">{t('admin.platform.rangeStart')}</Label>
                        <Input
                            id="np-start"
                            type="number"
                            min={1}
                            max={65535}
                            value={rangeStart}
                            onChange={e => setRangeStart(e.target.value)}
                            className="font-mono"
                        />
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="np-end">{t('admin.platform.rangeEnd')}</Label>
                        <Input
                            id="np-end"
                            type="number"
                            min={1}
                            max={65535}
                            value={rangeEnd}
                            onChange={e => setRangeEnd(e.target.value)}
                            className="font-mono"
                        />
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="np-reserved">{t('admin.platform.reservedPorts')}</Label>
                        <Input
                            id="np-reserved"
                            value={reserved}
                            onChange={e => setReserved(e.target.value)}
                            placeholder="30100,30200"
                            className="font-mono"
                        />
                        <p className="text-xs text-muted-foreground">{t('admin.platform.reservedPortsHelp')}</p>
                    </div>
                </div>
                <Button onClick={handleSave} disabled={saving} size="sm" className="mt-4">
                    <Save className="mr-2 h-4 w-4" />{saving ? t('common.saving') : t('common.save')}
                </Button>
            </CardContent>
        </Card>
    );
}

// ── Nodeport config keys to hide from the generic table ──────────────────────

const NODEPORT_KEYS = new Set(['nodeport_range_start', 'nodeport_range_end', 'nodeport_reserved']);

// ── Main Component ───────────────────────────────────────────────────────────

export default function AdminPlatformConfigTab({ initialConfig }: { initialConfig: ConfigEntry[] }) {
    const t = useT();
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
            toast.success(t('admin.platform.configSaved'));
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

    // Filter out nodeport keys from generic list — they have their own card
    const genericConfig = config.filter(c => !NODEPORT_KEYS.has(c.key));

    return (
        <>
            <NodePortConfigCard initialConfig={config} />

            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{t('admin.platform.title')}</CardTitle>
                        <CardDescription>{t('admin.platform.description')}</CardDescription>
                    </div>
                    <Button size="sm" onClick={openAdd}><Plus className="mr-2 h-4 w-4" />{t('common.add')}</Button>
                </CardHeader>
                <CardContent>
                    {genericConfig.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t('admin.platform.empty')}</p>
                    ) : (
                        <div className="space-y-2">
                            {genericConfig.map((entry) => (
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
                        <DialogTitle>{editing ? t('admin.platform.editEntry') : t('admin.platform.addEntry')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t('admin.platform.key')}</Label>
                            <Input
                                value={form.key}
                                onChange={e => setForm(f => ({ ...f, key: e.target.value }))}
                                disabled={!!editing}
                                className="font-mono"
                                placeholder="MY_CONFIG_KEY"
                            />
                        </div>
                        <div className="space-y-2">
                            <Label>{t('admin.platform.value')}</Label>
                            <Input
                                value={form.value}
                                onChange={e => setForm(f => ({ ...f, value: e.target.value }))}
                                placeholder="value"
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => { setEditing(null); setAdding(false); }}>{t('common.cancel')}</Button>
                        <Button onClick={handleSave} disabled={saving}>{saving ? t('common.saving') : t('common.save')}</Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
