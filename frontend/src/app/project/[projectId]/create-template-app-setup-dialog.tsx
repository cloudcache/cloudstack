'use client'

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import {
    Form,
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { zodResolver } from "@hookform/resolvers/zod"
import { useForm } from "react-hook-form"
import { useTransition } from 'react'
import { useEffect, useState } from "react";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { Button } from "@/components/ui/button";
import { toast } from "sonner"
import { AppTemplateModel, appTemplateZodModel } from "@/shared/model/app-template.model"
import { createAppFromTemplate, loadBindingChoices } from "./actions"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useT } from "@/i18n"
import type { TemplateDto, TemplateRequirement, TemplateBindingChoice } from "@/server/adapter/backend-api.adapter"

// Per-requirement local UI state.
// Note: a declared requirement is mandatory at deploy time — there is no
// 'skip' mode. User must pick managed or provision and a concrete ref.
type BindingFormState = {
    mode: 'managed' | 'provision';
    managed_ref_id?: string;
    provision_cluster_id?: string;
    provision_name_hint?: string;
};

export default function CreateTemplateAppSetupDialog({
    appTemplate,
    templateDto,
    projectId,
    dialogClosed
}: {
    appTemplate?: AppTemplateModel;
    templateDto?: TemplateDto;  // carries requirements + id for binding submission
    projectId: string;
    dialogClosed?: () => void;
}) {
    const t = useT();
    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [submitting, startTransition] = useTransition();
    const [submitError, setSubmitError] = useState<string>('');

    // Form for app name + input fields
    const form = useForm<AppTemplateModel>({
        resolver: zodResolver(appTemplateZodModel) as any,
        defaultValues: appTemplate,
    });

    // Bindings state, keyed by requirement.key
    const [bindings, setBindings] = useState<Record<string, BindingFormState>>({});
    const [choices, setChoices] = useState<{
        databases: any[]; dbClusters: any[]; s3Targets: any[];
        mqEndpoints: any[]; smtpEndpoints: any[]; redisEndpoints: any[];
    }>({ databases: [], dbClusters: [], s3Targets: [], mqEndpoints: [], smtpEndpoints: [], redisEndpoints: [] });

    // Reset on template change
    useEffect(() => {
        setIsOpen(!!appTemplate && !!projectId);
        form.reset(appTemplate);
        setSubmitError('');
        // Initialize bindings from requirements (declared = mandatory, default to first allowed mode)
        const reqs: TemplateRequirement[] = (templateDto?.requirements ?? []) as TemplateRequirement[];
        const initial: Record<string, BindingFormState> = {};
        for (const r of reqs) {
            const modes = r.binding_modes ?? ['managed'];
            initial[r.key] = {
                mode: (modes[0] as any) ?? 'managed',
                provision_name_hint: r.key,
            };
        }
        setBindings(initial);
    }, [appTemplate, templateDto, projectId]);

    // Load binding choices once when dialog opens with requirements
    useEffect(() => {
        if (!isOpen || !projectId) return;
        const reqs = templateDto?.requirements ?? [];
        if (reqs.length === 0) return;
        loadBindingChoices(projectId).then(setChoices).catch(() => undefined);
    }, [isOpen, projectId, templateDto]);

    const updateBinding = (key: string, patch: Partial<BindingFormState>) => {
        setBindings(prev => ({ ...prev, [key]: { ...prev[key], ...patch } }));
    };

    const onSubmit = (data: AppTemplateModel) => {
        if (!templateDto) {
            toast.error("Template metadata missing.");
            return;
        }
        const template = data.templates?.[0];
        const app_name = template?.appModel?.name ?? '';
        if (!app_name) {
            toast.error("App name is required.");
            return;
        }

        // Collect input overrides (keyed by input.key)
        const input_overrides: Record<string, any> = {};
        for (const input of template?.inputSettings ?? []) {
            input_overrides[input.key] = input.value;
        }

        // Build bindings payload — every declared requirement must be present
        const bindingsPayload: TemplateBindingChoice[] = Object.entries(bindings)
            .map(([requirement_key, b]) => ({
                requirement_key,
                mode: b.mode,
                managed_ref_id: b.managed_ref_id,
                provision_cluster_id: b.provision_cluster_id,
                provision_name_hint: b.provision_name_hint,
            }));

        startTransition(async () => {
            const result = await createAppFromTemplate(null, {
                template_id: templateDto.id,
                app_name,
                display_name: app_name,
                bindings: bindingsPayload,
                input_overrides,
            }, projectId);
            if (result?.status === 'success') {
                toast.success(t('templates.appCreated'), { description: t('templates.deployToStartApp') });
                form.reset();
                setIsOpen(false);
                dialogClosed?.();
            } else {
                setSubmitError(result?.message ?? 'Failed to create app');
            }
        });
    };

    const requirements: TemplateRequirement[] = (templateDto?.requirements ?? []) as TemplateRequirement[];

    return (
        <Dialog open={!!isOpen} onOpenChange={(isOpened) => {
            setIsOpen(isOpened);
            if (!isOpened) dialogClosed?.();
        }}>
            <DialogContent className="sm:max-w-[600px]">
                <DialogHeader>
                    <DialogTitle>{t('templates.createNamedApp', { name: appTemplate?.name ?? '' })}</DialogTitle>
                    <DialogDescription>{t('templates.insertValues')}</DialogDescription>
                </DialogHeader>
                <ScrollArea className="max-h-[70vh]">
                    <div className="px-2">
                        <Form {...form}>
                            <form onSubmit={form.handleSubmit(onSubmit)}>
                                <div className="space-y-6">
                                    {appTemplate?.templates.map((template, templateIndex) => (
                                        <div key={templateIndex} className="space-y-4">
                                            {appTemplate.templates.length > 1 && templateIndex > 0 && <div className="border-t pt-4" />}
                                            <FormField
                                                control={form.control}
                                                name={`templates.${templateIndex}.appModel.name` as any}
                                                render={({ field }) => (
                                                    <FormItem>
                                                        <FormLabel>{t('templates.appName')}</FormLabel>
                                                        <FormControl><Input {...field} /></FormControl>
                                                        <FormMessage />
                                                    </FormItem>
                                                )}
                                            />
                                            {template.inputSettings.map((input, settingsIndex) => (
                                                <FormField key={settingsIndex}
                                                    control={form.control}
                                                    name={`templates.${templateIndex}.inputSettings.${settingsIndex}.value` as any}
                                                    render={({ field }) => (
                                                        <FormItem>
                                                            <FormLabel>{input.label}</FormLabel>
                                                            <FormControl><Input {...field} /></FormControl>
                                                            {input.randomGeneratedIfEmpty &&
                                                                <FormDescription>{t('templates.randomIfEmpty')}</FormDescription>}
                                                            <FormMessage />
                                                        </FormItem>
                                                    )}
                                                />
                                            ))}
                                        </div>
                                    ))}

                                    {/* ── Bindings section (Phase 2a) ── */}
                                    {requirements.length > 0 && (
                                        <div className="border-t pt-4 space-y-4">
                                            <div className="text-sm font-semibold">Service Dependencies</div>
                                            {requirements.map((req) => {
                                                const b = bindings[req.key] ?? { mode: 'managed' };
                                                // What modes does this kind support?
                                                // Per backend: only database supports provision; everything else managed-only.
                                                const supportsProvision = req.kind === 'database';
                                                const declaredModes = req.binding_modes ?? (supportsProvision ? ['managed', 'provision'] : ['managed']);
                                                const modes = declaredModes.filter(m => m === 'managed' || (m === 'provision' && supportsProvision));

                                                // Which list to show under "managed"?
                                                const managedList: any[] = (() => {
                                                    switch (req.kind) {
                                                        case 'database': return choices.databases;
                                                        case 'objstore': return choices.s3Targets;
                                                        case 'mq': return choices.mqEndpoints;
                                                        case 'smtp': return choices.smtpEndpoints;
                                                        case 'cache': return choices.redisEndpoints;
                                                        default: return [];
                                                    }
                                                })();
                                                const managedPlaceholder = (() => {
                                                    switch (req.kind) {
                                                        case 'database': return 'Select database';
                                                        case 'objstore': return 'Select S3 target';
                                                        case 'mq': return 'Select MQ endpoint';
                                                        case 'smtp': return 'Select SMTP relay';
                                                        case 'cache': return 'Select Redis endpoint';
                                                        default: return 'Select…';
                                                    }
                                                })();

                                                return (
                                                    <div key={req.key} className="border rounded-md p-3 space-y-3">
                                                        <div className="flex items-center justify-between">
                                                            <div>
                                                                <div className="font-medium">{req.label ?? req.key}</div>
                                                                <div className="text-xs text-muted-foreground">{req.kind}{req.engine ? ` · ${req.engine}` : ''} · required</div>
                                                            </div>
                                                            {modes.length > 1 && (
                                                                <Select
                                                                    value={b.mode}
                                                                    onValueChange={(v) => updateBinding(req.key, { mode: v as any })}>
                                                                    <SelectTrigger className="w-44">
                                                                        <SelectValue />
                                                                    </SelectTrigger>
                                                                    <SelectContent>
                                                                        {modes.includes('managed') && <SelectItem value="managed">Bind existing</SelectItem>}
                                                                        {modes.includes('provision') && <SelectItem value="provision">Provision new</SelectItem>}
                                                                    </SelectContent>
                                                                </Select>
                                                            )}
                                                        </div>

                                                        {b.mode === 'managed' && (
                                                            <Select
                                                                value={b.managed_ref_id ?? ''}
                                                                onValueChange={(v) => updateBinding(req.key, { managed_ref_id: v })}>
                                                                <SelectTrigger>
                                                                    <SelectValue placeholder={managedPlaceholder} />
                                                                </SelectTrigger>
                                                                <SelectContent>
                                                                    {managedList.map((item: any) => (
                                                                        <SelectItem key={item.id} value={item.id}>
                                                                            {item.name ?? item.db_name ?? item.id}
                                                                            {item.cluster_name ? ` (${item.cluster_name})` : ''}
                                                                        </SelectItem>
                                                                    ))}
                                                                </SelectContent>
                                                            </Select>
                                                        )}

                                                        {b.mode === 'provision' && (
                                                            <div className="space-y-2">
                                                                <Select
                                                                    value={b.provision_cluster_id ?? ''}
                                                                    onValueChange={(v) => updateBinding(req.key, { provision_cluster_id: v })}>
                                                                    <SelectTrigger>
                                                                        <SelectValue placeholder="Select cluster to provision on" />
                                                                    </SelectTrigger>
                                                                    <SelectContent>
                                                                        {choices.dbClusters
                                                                            .filter((c: any) => !req.engine || c.cluster_type?.toLowerCase().includes(req.engine.toLowerCase()))
                                                                            .map((c: any) => (
                                                                                <SelectItem key={c.id} value={c.id}>
                                                                                    {c.name} ({c.cluster_type})
                                                                                </SelectItem>
                                                                            ))}
                                                                    </SelectContent>
                                                                </Select>
                                                                <Input
                                                                    placeholder="DB name suffix (e.g. main)"
                                                                    value={b.provision_name_hint ?? ''}
                                                                    onChange={(e) => updateBinding(req.key, { provision_name_hint: e.target.value })} />
                                                            </div>
                                                        )}
                                                    </div>
                                                );
                                            })}
                                        </div>
                                    )}

                                    {submitError && <p className="text-red-500">{submitError}</p>}
                                    <Button type="submit" disabled={submitting}>
                                        {submitting ? t('common.saving') : t('common.create')}
                                    </Button>
                                </div>
                            </form>
                        </Form>
                    </div>
                </ScrollArea>
            </DialogContent>
        </Dialog>
    );
}
