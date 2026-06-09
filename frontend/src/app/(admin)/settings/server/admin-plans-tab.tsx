'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
// Table import removed — using card layout for better visibility
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { createPlan, updatePlan, deletePlan } from "./actions";
import { Plus, Pencil, Trash } from "lucide-react";
import { useT } from "@/i18n";

interface Plan {
    id: string;
    name: string;
    display_name: string;
    description?: string;
    price_monthly: number;
    price_annually?: number;
    // Backend returns quotas nested: { quota: { cpu_mcores, mem_mb, ... } }
    quota?: {
        cpu_mcores?: number; mem_mb?: number; storage_gb?: number;
        bandwidth_gb?: number; domain_count?: number; db_instance_count?: number;
        project_count?: number; app_count?: number; request_million?: number;
    };
    // Flat fallback (for create/update forms)
    quota_cpu_mcores?: number;
    quota_mem_mb?: number;
    quota_storage_gb?: number;
    quota_bandwidth_gb?: number;
    quota_domain_count?: number;
    quota_db_instance_count?: number;
    quota_project_count?: number;
    quota_app_count?: number;
    quota_request_million?: number;
    is_active: boolean;
    is_public: boolean;
    sort_order: number;
}

/** Helper to read a quota value from either nested or flat format */
function q(plan: Plan, key: string): number {
    return (plan.quota as any)?.[key] ?? (plan as any)[`quota_${key}`] ?? 0;
}

const emptyForm = {
    name: '', display_name: '', description: '',
    price_monthly: 0, price_annually: 0,
    quota_cpu_mcores: 2000, quota_mem_mb: 2048, quota_storage_gb: 10,
    quota_bandwidth_gb: 50, quota_domain_count: 3, quota_db_instance_count: 2,
    quota_project_count: 2, quota_app_count: 5, quota_request_million: 1,
    // P2c managed-binding quotas (0 = unlimited)
    quota_mq_binding_count: 0, quota_smtp_binding_count: 0,
    quota_redis_binding_count: 0, quota_s3_binding_count: 0,
    is_public: true, sort_order: 0,
};

export default function AdminPlansTab({ initialItems }: { initialItems: Plan[] }) {
    const t = useT();
    const [items, setItems] = useState<Plan[]>(initialItems);
    const [showForm, setShowForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [form, setForm] = useState(emptyForm);
    const [saving, setSaving] = useState(false);
    const { openConfirmDialog } = useConfirmDialog();

    const openAdd = () => {
        setEditingId(null);
        setForm(emptyForm);
        setShowForm(true);
    };

    const openEdit = (plan: Plan) => {
        setEditingId(plan.id);
        setForm({
            name: plan.name, display_name: plan.display_name, description: plan.description ?? '',
            price_monthly: typeof plan.price_monthly === 'string' ? parseFloat(plan.price_monthly) : plan.price_monthly,
            price_annually: plan.price_annually ? (typeof plan.price_annually === 'string' ? parseFloat(plan.price_annually as any) : plan.price_annually) : 0,
            quota_cpu_mcores: q(plan, 'cpu_mcores'), quota_mem_mb: q(plan, 'mem_mb'),
            quota_storage_gb: q(plan, 'storage_gb'), quota_bandwidth_gb: q(plan, 'bandwidth_gb'),
            quota_domain_count: q(plan, 'domain_count'), quota_db_instance_count: q(plan, 'db_instance_count'),
            quota_project_count: q(plan, 'project_count'), quota_app_count: q(plan, 'app_count'),
            quota_request_million: q(plan, 'request_million'),
            quota_mq_binding_count: q(plan, 'mq_binding_count'),
            quota_smtp_binding_count: q(plan, 'smtp_binding_count'),
            quota_redis_binding_count: q(plan, 'redis_binding_count'),
            quota_s3_binding_count: q(plan, 's3_binding_count'),
            is_public: plan.is_public, sort_order: plan.sort_order,
        });
        setShowForm(true);
    };

    const handleSave = async () => {
        setSaving(true);
        if (editingId) {
            await Toast.fromAction(() => updatePlan(editingId, form));
        } else {
            await Toast.fromAction(() => createPlan(form));
        }
        setSaving(false);
        setShowForm(false);
        window.location.reload();
    };

    const handleDelete = async (plan: Plan) => {
        const confirmed = await openConfirmDialog({
            title: t('plans.deletePlan'),
            description: t('plans.deleteDescription', { name: plan.display_name }),
            okButton: t('common.delete'), cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        await Toast.fromAction(() => deletePlan(plan.id));
        setItems(prev => prev.filter(i => i.id !== plan.id));
    };

    const handleToggleActive = async (plan: Plan) => {
        await Toast.fromAction(() => updatePlan(plan.id, { is_active: !plan.is_active }));
        setItems(prev => prev.map(p => p.id === plan.id ? { ...p, is_active: !p.is_active } : p));
    };

    const set = (key: string, value: any) => setForm(f => ({ ...f, [key]: value }));

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>{t('plans.subscriptionPlans')}</CardTitle>
                        <CardDescription>{t('plans.adminDescription')}</CardDescription>
                    </div>
                    <Button size="sm" onClick={openAdd}><Plus className="mr-2 h-4 w-4" />{t('plans.addPlan')}</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t('plans.noPlans')}</p>
                    ) : (
                        <div className="space-y-4">
                            {items.map(plan => (
                                <div key={plan.id} className="border rounded-lg p-4 space-y-3">
                                    <div className="flex items-center justify-between">
                                        <div>
                                            <div className="font-semibold text-lg">{plan.display_name}</div>
                                            <div className="text-sm text-muted-foreground">{plan.name} &middot; ¥{plan.price_monthly}/mo</div>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <Badge variant={plan.is_active ? 'default' : 'secondary'}
                                                className="cursor-pointer" onClick={() => handleToggleActive(plan)}>
                                                {plan.is_active ? t('plans.active') : t('plans.inactive')}
                                            </Badge>
                                            {!plan.is_public && <Badge variant="outline">{t('plans.private')}</Badge>}
                                        </div>
                                    </div>
                                    {plan.description && <p className="text-sm text-muted-foreground">{plan.description}</p>}
                                    <div className="grid grid-cols-3 md:grid-cols-5 gap-2 text-sm">
                                        <div><span className="text-muted-foreground">{t('plans.cpu')}:</span> {(q(plan, 'cpu_mcores') / 1000).toFixed(1)}c</div>
                                        <div><span className="text-muted-foreground">{t('plans.memoryShort')}:</span> {(q(plan, 'mem_mb') / 1024).toFixed(1)}G</div>
                                        <div><span className="text-muted-foreground">{t('plans.storage')}:</span> {q(plan, 'storage_gb')}G</div>
                                        <div><span className="text-muted-foreground">{t('plans.apps')}:</span> {q(plan, 'app_count')}</div>
                                        <div><span className="text-muted-foreground">{t('plans.projects')}:</span> {q(plan, 'project_count')}</div>
                                        <div><span className="text-muted-foreground">{t('plans.domains')}:</span> {q(plan, 'domain_count')}</div>
                                        <div><span className="text-muted-foreground">{t('plans.databasesShort')}:</span> {q(plan, 'db_instance_count')}</div>
                                        <div><span className="text-muted-foreground">{t('plans.bandwidthShort')}:</span> {q(plan, 'bandwidth_gb')}G</div>
                                        <div><span className="text-muted-foreground">{t('plans.requestsShort')}:</span> {q(plan, 'request_million')}M</div>
                                    </div>
                                    <div className="flex gap-2 pt-1">
                                        <Button size="sm" variant="outline" onClick={() => openEdit(plan)}>
                                            <Pencil className="mr-1 h-3 w-3" /> {t('common.edit')}
                                        </Button>
                                        <Button size="sm" variant="outline" onClick={() => handleDelete(plan)}>
                                            <Trash className="mr-1 h-3 w-3 text-destructive" /> {t('common.delete')}
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            <Dialog open={showForm} onOpenChange={setShowForm}>
                <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
                    <DialogHeader>
                        <DialogTitle>{editingId ? t('plans.editPlan') : t('plans.createPlan')}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                                <Label>{t('plans.nameSlug')}</Label>
                                <Input value={form.name} onChange={e => set('name', e.target.value)} placeholder="pro" disabled={!!editingId} />
                            </div>
                            <div className="space-y-2">
                                <Label>{t('plans.displayName')}</Label>
                                <Input value={form.display_name} onChange={e => set('display_name', e.target.value)} placeholder="Professional" />
                            </div>
                        </div>
                        <div className="space-y-2">
                            <Label>{t('common.description')}</Label>
                            <Textarea value={form.description} onChange={e => set('description', e.target.value)} placeholder={t('plans.descriptionPlaceholder')} rows={2} />
                        </div>
                        <div className="grid grid-cols-3 gap-4">
                            <div className="space-y-2">
                                <Label>{t('plans.priceMonthly')}</Label>
                                <Input type="number" value={form.price_monthly} onChange={e => set('price_monthly', parseFloat(e.target.value) || 0)} />
                            </div>
                            <div className="space-y-2">
                                <Label>{t('plans.priceAnnually')}</Label>
                                <Input type="number" value={form.price_annually} onChange={e => set('price_annually', parseFloat(e.target.value) || 0)} />
                            </div>
                            <div className="space-y-2">
                                <Label>{t('plans.sortOrder')}</Label>
                                <Input type="number" value={form.sort_order} onChange={e => set('sort_order', parseInt(e.target.value) || 0)} />
                            </div>
                        </div>
                        <div className="border rounded-lg p-4 space-y-3">
                            <h4 className="font-medium text-sm">{t('plans.quotas')}</h4>
                            <div className="grid grid-cols-3 gap-4">
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.cpuMcores')}</Label>
                                    <Input type="number" value={form.quota_cpu_mcores} onChange={e => set('quota_cpu_mcores', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.memoryMb')}</Label>
                                    <Input type="number" value={form.quota_mem_mb} onChange={e => set('quota_mem_mb', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.storageGb')}</Label>
                                    <Input type="number" value={form.quota_storage_gb} onChange={e => set('quota_storage_gb', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.bandwidthGb')}</Label>
                                    <Input type="number" value={form.quota_bandwidth_gb} onChange={e => set('quota_bandwidth_gb', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.domains')}</Label>
                                    <Input type="number" value={form.quota_domain_count} onChange={e => set('quota_domain_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.dbInstances')}</Label>
                                    <Input type="number" value={form.quota_db_instance_count} onChange={e => set('quota_db_instance_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.projects')}</Label>
                                    <Input type="number" value={form.quota_project_count} onChange={e => set('quota_project_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.apps')}</Label>
                                    <Input type="number" value={form.quota_app_count} onChange={e => set('quota_app_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">{t('plans.requestsMonthly')}</Label>
                                    <Input type="number" value={form.quota_request_million} onChange={e => set('quota_request_million', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">MQ bindings</Label>
                                    <Input type="number" min={0} value={form.quota_mq_binding_count} onChange={e => set('quota_mq_binding_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">SMTP bindings</Label>
                                    <Input type="number" min={0} value={form.quota_smtp_binding_count} onChange={e => set('quota_smtp_binding_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">Redis bindings</Label>
                                    <Input type="number" min={0} value={form.quota_redis_binding_count} onChange={e => set('quota_redis_binding_count', parseInt(e.target.value) || 0)} />
                                </div>
                                <div className="space-y-1">
                                    <Label className="text-xs">S3 bindings</Label>
                                    <Input type="number" min={0} value={form.quota_s3_binding_count} onChange={e => set('quota_s3_binding_count', parseInt(e.target.value) || 0)} />
                                </div>
                            </div>
                            <p className="text-xs text-muted-foreground">Set 0 for unlimited.</p>
                        </div>
                        <div className="flex items-center gap-4">
                            <div className="flex items-center gap-2">
                                <Switch checked={form.is_public} onCheckedChange={v => set('is_public', v)} />
                                <Label>{t('plans.public')}</Label>
                            </div>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setShowForm(false)}>{t('common.cancel')}</Button>
                        <Button onClick={handleSave} disabled={saving || !form.name || !form.display_name}>
                            {saving ? t('common.saving') : editingId ? t('plans.updatePlan') : t('plans.createPlan')}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}
