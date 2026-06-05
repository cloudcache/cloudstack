'use client'

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import {
    Form,
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { zodResolver } from "@hookform/resolvers/zod"
import { useForm } from "react-hook-form"
import { useActionState, useTransition } from 'react'
import { useEffect, useState } from "react";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { SubmitButton } from "@/components/custom/submit-button";
import { AppDomain } from "@/shared/model/prisma-compat"
import { AppDomainEditModel, appDomainEditZodModel } from "@/shared/model/domain-edit.model"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { saveDomain, getQuickstackDomainSuffix } from "./actions"
import { toast } from "sonner"
import CheckboxFormField from "@/components/custom/checkbox-form-field"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { HostnameDnsProviderUtils } from "@/shared/utils/domain-dns-provider.utils"
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip"
import { useT } from "@/i18n"


export default function DialogEditDialog({ children, domain, appId }: { children: React.ReactNode; domain?: AppDomain; appId: string; }) {

    const t = useT();
    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [domainSuffix, setDomainSuffix] = useState<string | undefined>(undefined);
    const [activeTab, setActiveTab] = useState<'custom' | 'quickstack'>('custom');

    useEffect(() => {
        // Load the quickstack.me domain suffix when dialog opens
        if (isOpen) {
            getQuickstackDomainSuffix().then((res) => {
                if (res.status === 'success' && res.data) {
                    setDomainSuffix(res.data);
                }
            });
        }
    }, [isOpen]);

    // Determine which tab should be active based on the domain
    useEffect(() => {
        if (domain?.hostname && domainSuffix) {
            if (HostnameDnsProviderUtils.containsDnsProviderHostname(domain.hostname)) {
                setActiveTab('quickstack');
            } else {
                setActiveTab('custom');
            }
        }
    }, [domain, domainSuffix]);

    const form = useForm<AppDomainEditModel>({
        resolver: zodResolver(appDomainEditZodModel) as any,
        defaultValues: {
            ...domain,
            useSsl: domain?.useSsl === false ? false : true
        }
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: AppDomainEditModel) =>
        saveDomain(state, {
            ...payload,
            appId,
            id: domain?.id
        }), FormUtils.getInitialFormState<typeof appDomainEditZodModel>());

    useEffect(() => {
        if (state.status === 'success') {
            form.reset();
            toast.success(t('app.domains.saved'), {
                description: t('app.common.deployToApply'),
            });
            setIsOpen(false);
        }
        FormUtils.mapValidationErrorsToForm<typeof appDomainEditZodModel>(state, form as any);
    }, [state]);

    const values = form.watch();

    useEffect(() => {
        if (domain) {
            form.reset(domain);
        }
    }, [domain, form]);

    // Extract the custom prefix from quickstack.me domain when editing
    const getQuickstackPrefix = (hostname: string): string => {
        if (!hostname || !domainSuffix) return '';
        if (hostname.endsWith(`.${domainSuffix}`)) {
            return hostname.replace(`.${domainSuffix}`, '');
        }
        return '';
    };

    // Handle form submission
    const handleFormSubmit = (data: AppDomainEditModel) => {
        return startTransition(() => formAction(data));
    };

    return (
        <>
            <div onClick={() => setIsOpen(true)}>
                {children}
            </div>
            <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
                <DialogContent className="sm:max-w-[425px]">
                    <DialogHeader>
                        <DialogTitle>{t('app.domains.editTitle')}</DialogTitle>
                        <DialogDescription>
                            {t('app.domains.editDescription')}
                        </DialogDescription>
                    </DialogHeader>
                    <Form {...form}>
                        <form action={(e) => form.handleSubmit((data) => {
                            return handleFormSubmit(data);
                        })()}>
                            <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as 'custom' | 'quickstack')} className="w-full">
                                <TabsList className="grid w-full grid-cols-2">
                                    <TabsTrigger value="custom">{t('app.domains.customDomain')}</TabsTrigger>
                                    {!!domainSuffix && <TabsTrigger value="quickstack">{t('app.domains.quickstackDomain')}</TabsTrigger>}
                                </TabsList>

                                <TabsContent value="custom" className="space-y-4 mt-4">
                                    <FormField
                                        control={form.control}
                                        name="hostname"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.domains.hostname')}</FormLabel>
                                                <FormControl>
                                                    <Input placeholder="example.com" {...field} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <FormField
                                        control={form.control}
                                        name="port"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.domains.appPort')}</FormLabel>
                                                <FormControl>
                                                    <Input type="number" placeholder="ex. 80" {...field} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <CheckboxFormField form={form} name="useSsl" label={t('app.domains.useHttps')} />
                                    {values.useSsl && <CheckboxFormField form={form} name="redirectHttps" label={t('app.domains.redirectHttps')} />}
                                </TabsContent>

                                <TabsContent value="quickstack" className="space-y-4 mt-4">
                                    <FormField
                                        control={form.control}
                                        name="hostname"
                                        render={({ field }) => {
                                            const prefixValue = getQuickstackPrefix(field.value || '');
                                            return (
                                                <FormItem>
                                                    <FormLabel>{t('app.domains.domainPrefix')}</FormLabel>
                                                    <FormControl>
                                                        <div className="flex items-center gap-2">
                                                            <Input
                                                                placeholder="my-app"
                                                                value={prefixValue}
                                                                onChange={(e) => {
                                                                    const newPrefix = e.target.value;
                                                                    const fullHostname = newPrefix ? `${newPrefix}.${domainSuffix}` : '';
                                                                    field.onChange(fullHostname);
                                                                }}
                                                                onBlur={field.onBlur}
                                                                name={field.name}
                                                            />
                                                            <Tooltip>
                                                                <TooltipTrigger asChild>
                                                                    <span className="text-sm text-muted-foreground whitespace-nowrap">
                                                                        .{domainSuffix}
                                                                    </span>
                                                                </TooltipTrigger>
                                                                <TooltipContent>
                                                                    <p>{t('app.domains.quickstackTooltip')}</p>
                                                                </TooltipContent>
                                                            </Tooltip>
                                                        </div>
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            );
                                        }}
                                    />

                                    <FormField
                                        control={form.control}
                                        name="port"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.domains.appPort')}</FormLabel>
                                                <FormControl>
                                                    <Input type="number" placeholder="ex. 80" {...field} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <CheckboxFormField form={form} name="useSsl" label={t('app.domains.useHttps')} />
                                    {values.useSsl && <CheckboxFormField form={form} name="redirectHttps" label={t('app.domains.redirectHttps')} />}
                                </TabsContent>
                            </Tabs>

                            <div className="mt-4 space-y-4">
                                <p className="text-red-500">{state.message}</p>
                                <SubmitButton>{t('common.save')}</SubmitButton>
                            </div>
                        </form>
                    </Form >
                </DialogContent>
            </Dialog>
        </>
    )



}
