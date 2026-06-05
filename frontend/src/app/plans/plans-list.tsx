'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { subscribeToPlan, cancelSubscription, createPlan, updatePlan, deletePlan } from "./actions";
import { Check, Cpu, HardDrive, Globe, Database, FolderOpen, AppWindow, Zap, Plus, Pencil, Trash } from "lucide-react";
import { useT } from "@/i18n";

interface Plan {
    id: string;
    name: string;
    display_name: string;
    description?: string;
    price_monthly: number | string;
    price_annually?: number | string | null;
    is_active?: boolean;
    is_public?: boolean;
    sort_order?: number;
    quota?: {
        cpu_mcores?: number; mem_mb?: number; storage_gb?: number;
        bandwidth_gb?: number; domain_count?: number; db_instance_count?: number;
        project_count?: number; app_count?: number; request_million?: number;
    };
}

interface Subscription {
    id: string;
    plan_id: string;
    plan_name?: string;
    plan_display_name?: string;
    status: string;
    billing_cycle: string;
    expires_at?: string;
}

function pq(plan: Plan, key: string): number {
    return (plan.quota as any)?.[key] ?? 0;
}

function price(v: number | string | null | undefined): number {
    if (v == null) return 0;
    return typeof v === 'string' ? parseFloat(v) || 0 : v;
}

const emptyForm = {
    name: '', display_name: '', description: '',
    price_monthly: 0, price_annually: 0,
    quota_cpu_mcores: 2000, quota_mem_mb: 2048, quota_storage_gb: 10,
    quota_bandwidth_gb: 50, quota_domain_count: 3, quota_db_instance_count: 2,
    quota_project_count: 2, quota_app_count: 5, quota_request_million: 1,
    is_public: true, sort_order: 0,
};

export default function PlansList({ plans, currentSubscription, isAdmin }: {
    plans: Plan[];
    currentSubscription: Subscription | null;
    isAdmin?: boolean;
}) {
    const t = useT();
    const [items, setItems] = useState<Plan[]>(plans);
    const [loading, setLoading] = useState<string | null>(null);
    const { openConfirmDialog } = useConfirmDialog();

    // Admin form state
    const [showForm, setShowForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [form, setForm] = useState(emptyForm);
    const [saving, setSaving] = useState(false);

    // ── User actions ──

    const handleSubscribe = async (plan: Plan) => {
        const p = price(plan.price_monthly);
        const confirmed = await openConfirmDialog({
            title: t('plans.subscribeTo', { name: plan.display_name }),
            description: p > 0
                ? t('plans.subscribePaidDescription', { name: plan.display_name, price: p })
                : t('plans.subscribeFreeDescription', { name: plan.display_name }),
            okButton: t('plans.subscribe'),
            cancelButton: t('common.cancel'),
        });
        if (!confirmed) return;
        setLoading(plan.id);
        await Toast.fromAction(() => subscribeToPlan(plan.id, 'MONTHLY'));
        setLoading(null);
        window.location.reload();
    };

    const handleCancel = async () => {
        const confirmed = await openConfirmDialog({
            title: t('plans.cancelSubscription'),
            description: t('plans.cancelSubscriptionDescription'),
            okButton: t('plans.cancelSubscription'),
            cancelButton: t('plans.keepSubscription'),
        });
        if (!confirmed) return;
        await Toast.fromAction(() => cancelSubscription());
        window.location.reload();
    };

    // ── Admin actions ──

    const openAdd = () => {
        setEditingId(null);
        setForm(emptyForm);
        setShowForm(true);
    };

    const openEdit = (plan: Plan) => {
        setEditingId(plan.id);
        setForm({
            name: plan.name, display_name: plan.display_name, description: plan.description ?? '',
            price_monthly: price(plan.price_monthly), price_annually: price(plan.price_annually),
            quota_cpu_mcores: pq(plan, 'cpu_mcores'), quota_mem_mb: pq(plan, 'mem_mb'),
            quota_storage_gb: pq(plan, 'storage_gb'), quota_bandwidth_gb: pq(plan, 'bandwidth_gb'),
            quota_domain_count: pq(plan, 'domain_count'), quota_db_instance_count: pq(plan, 'db_instance_count'),
            quota_project_count: pq(plan, 'project_count'), quota_app_count: pq(plan, 'app_count'),
            quota_request_million: pq(plan, 'request_million'),
            is_public: plan.is_public ?? true, sort_order: plan.sort_order ?? 0,
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

    const isCurrentPlan = (planId: string) =>
        currentSubscription?.plan_id === planId && currentSubscription?.status === 'ACTIVE';

    const set = (key: string, value: any) => setForm(f => ({ ...f, [key]: value }));

    return (
        <div className="space-y-6">
            {/* Current subscription banner */}
            {currentSubscription && currentSubscription.status === 'ACTIVE' && (
                <Card className="border-primary">
                    <CardHeader className="pb-3">
                        <div className="flex items-center justify-between">
                            <div>
                                <CardTitle className="text-base">{t('plans.currentSubscription')}</CardTitle>
                                <CardDescription>
                                    {t('plans.plan')}: <strong>{currentSubscription.plan_display_name ?? currentSubscription.plan_name ?? t('common.unknown')}</strong> &middot;{' '}
                                    {t('plans.cycle')}: {currentSubscription.billing_cycle} &middot;{' '}
                                    {t('common.status')}: <Badge variant="default">{currentSubscription.status}</Badge>
                                    {currentSubscription.expires_at && (
                                        <> &middot; {t('plans.expires')}: {new Date(currentSubscription.expires_at).toLocaleDateString()}</>
                                    )}
                                </CardDescription>
                            </div>
                            <Button variant="outline" size="sm" onClick={handleCancel}>
                                {t('plans.cancelSubscription')}
                            </Button>
                        </div>
                    </CardHeader>
                </Card>
            )}

            {/* Admin: Add Plan button */}
            {isAdmin && (
                <div className="flex justify-end">
                    <Button onClick={openAdd}><Plus className="mr-2 h-4 w-4" />{t('plans.addPlan')}</Button>
                </div>
            )}

            {/* Plan cards */}
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
                {items.map(plan => (
                    <Card key={plan.id} className={
                        isCurrentPlan(plan.id) ? 'border-primary ring-1 ring-primary' :
                        (isAdmin && plan.is_active === false) ? 'opacity-60' : ''
                    }>
                        <CardHeader>
                            <div className="flex items-center justify-between">
                                <CardTitle>{plan.display_name}</CardTitle>
                                <div className="flex gap-1">
                                    {isCurrentPlan(plan.id) && <Badge>{t('plans.current')}</Badge>}
                                    {isAdmin && plan.is_active === false && <Badge variant="secondary">{t('plans.inactive')}</Badge>}
                                    {isAdmin && plan.is_public === false && <Badge variant="outline">{t('plans.private')}</Badge>}
                                </div>
                            </div>
                            {plan.description && <CardDescription>{plan.description}</CardDescription>}
                        </CardHeader>
                        <CardContent>
                            <div className="mb-4">
                                {(() => {
                                    const p = price(plan.price_monthly);
                                    const ap = price(plan.price_annually);
                                    return <>
                                        <span className="text-3xl font-bold">{p > 0 ? `¥${p}` : t('plans.free')}</span>
                                        {p > 0 && <span className="text-muted-foreground">{t('plans.perMonth')}</span>}
                                        {ap > 0 && <div className="text-sm text-muted-foreground mt-1">{t('plans.orPerYear', { price: ap })}</div>}
                                    </>;
                                })()}
                            </div>
                            <div className="space-y-2 text-sm">
                                <QuotaLine icon={Cpu} label={t('plans.cpu')} value={t('admin.nodes.cores', { value: (pq(plan, 'cpu_mcores') / 1000).toFixed(1) })} />
                                <QuotaLine icon={Cpu} label={t('admin.nodes.memory')} value={t('admin.nodes.gb', { value: (pq(plan, 'mem_mb') / 1024).toFixed(1) })} />
                                <QuotaLine icon={HardDrive} label={t('plans.storage')} value={t('admin.nodes.gb', { value: pq(plan, 'storage_gb') })} />
                                <QuotaLine icon={Globe} label={t('plans.bandwidthShort')} value={t('admin.nodes.gb', { value: pq(plan, 'bandwidth_gb') })} />
                                <QuotaLine icon={Globe} label={t('plans.domains')} value={`${pq(plan, 'domain_count')}`} />
                                <QuotaLine icon={Database} label={t('plans.databases')} value={`${pq(plan, 'db_instance_count')}`} />
                                <QuotaLine icon={FolderOpen} label={t('plans.projects')} value={`${pq(plan, 'project_count')}`} />
                                <QuotaLine icon={AppWindow} label={t('plans.apps')} value={`${pq(plan, 'app_count')}`} />
                                <QuotaLine icon={Zap} label={t('plans.requests')} value={t('plans.millionPerMonth', { value: pq(plan, 'request_million') })} />
                            </div>
                        </CardContent>
                        <CardFooter className="flex flex-col gap-2">
                            {/* Subscribe / Current button */}
                            {isCurrentPlan(plan.id) ? (
                                <Button disabled className="w-full" variant="outline">
                                    <Check className="mr-2 h-4 w-4" /> {t('plans.currentPlan')}
                                </Button>
                            ) : (
                                <Button
                                    className="w-full"
                                    onClick={() => handleSubscribe(plan)}
                                    disabled={loading === plan.id}
                                >
                                    {loading === plan.id ? t('plans.subscribing') : currentSubscription?.status === 'ACTIVE' ? t('plans.switchPlan') : t('plans.subscribe')}
                                </Button>
                            )}

                            {/* Admin: Edit / Delete / Toggle */}
                            {isAdmin && (
                                <div className="flex gap-2 w-full">
                                    <Button size="sm" variant="outline" className="flex-1" onClick={() => openEdit(plan)}>
                                        <Pencil className="mr-1 h-3 w-3" /> {t('common.edit')}
                                    </Button>
                                    <Button size="sm" variant="outline" onClick={() => handleToggleActive(plan)}>
                                        {plan.is_active === false ? t('plans.activate') : t('plans.deactivate')}
                                    </Button>
                                    <Button size="sm" variant="outline" onClick={() => handleDelete(plan)}>
                                        <Trash className="h-3 w-3 text-destructive" />
                                    </Button>
                                </div>
                            )}
                        </CardFooter>
                    </Card>
                ))}
            </div>

            {items.length === 0 && (
                <p className="text-muted-foreground text-center py-12">
                    {isAdmin ? t('plans.noPlansAdmin') : t('plans.noPlansUser')}
                </p>
            )}

            {/* Admin: Create/Edit Plan Dialog */}
            {isAdmin && (
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
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                <Switch checked={form.is_public} onCheckedChange={v => set('is_public', v)} />
                                <Label>{t('plans.publicVisible')}</Label>
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
            )}
        </div>
    );
}

function QuotaLine({ icon: Icon, label, value }: { icon: any; label: string; value: string }) {
    return (
        <div className="flex items-center gap-2">
            <Icon className="h-4 w-4 text-muted-foreground shrink-0" />
            <span className="text-muted-foreground">{label}:</span>
            <span className="font-medium ml-auto">{value}</span>
        </div>
    );
}
