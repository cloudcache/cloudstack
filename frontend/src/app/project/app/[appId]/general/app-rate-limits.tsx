'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { saveGeneralAppRateLimits } from "./actions";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { Input } from "@/components/ui/input";
import { AppRateLimitsModel, appRateLimitsZodModel } from "@/shared/model/app-rate-limits.model";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { cn } from "@/frontend/utils/utils";
import { getRessourceDataApp } from "../overview/actions";
import { PodsResourceInfoModel } from "@/shared/model/pods-resource-info.model";
import { KubeSizeConverter } from "@/shared/utils/kubernetes-size-converter.utils";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/i18n";


export default function GeneralAppRateLimits({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {
    const t = useT();
    const form = useForm<AppRateLimitsModel>({
        resolver: zodResolver(appRateLimitsZodModel) as any,
        defaultValues: app,
        disabled: readonly
    });

    const [monitoringData, setMonitoringData] = useState<PodsResourceInfoModel | undefined>(undefined);

    useEffect(() => {
        getRessourceDataApp(app.projectId, app.id).then((res) => {
            if (res.status === 'success' && res.data) {
                setMonitoringData(res.data);
            }
        }).catch(() => { /* pod may not be running, silently ignore */ });
    }, [app.id, app.projectId]);

    const suggestedMemoryMb = monitoringData && monitoringData.ramAbsolutBytes
        ? Math.ceil(KubeSizeConverter.fromBytesToMegabytes(monitoringData.ramAbsolutBytes))
        : undefined;
    const suggestedCpuMillicores = monitoringData && monitoringData.cpuAbsolutCores
        ? Math.max(1, Math.round(monitoringData.cpuAbsolutCores * 1000))
        : undefined;

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: AppRateLimitsModel) => saveGeneralAppRateLimits(state, payload, app.projectId, app.id), FormUtils.getInitialFormState<typeof appRateLimitsZodModel>());
    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('app.rateLimits.saved'), {
                description: t('app.common.deployToApply'),
            });
        }
        FormUtils.mapValidationErrorsToForm<typeof appRateLimitsZodModel>(state, form as any);
    }, [state]);

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.rateLimits.title')}</CardTitle>
                <CardDescription>{t('app.rateLimits.description')}</CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                    return startTransition(() => formAction(data));
                })()}>
                    <CardContent className="space-y-4">
                        <div className={cn('grid grid-cols-2 gap-4 ', app.appType !== 'APP' && 'hidden')}>

                            <FormField
                                control={form.control}
                                name="replicas"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.rateLimits.replicaCount')}</FormLabel>
                                        <FormControl>
                                            <Input type="number" {...field} value={field.value} />
                                        </FormControl>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />
                        </div>
                        <div className="grid grid-cols-2 gap-4">

                            <FormField
                                control={form.control}
                                name="memoryLimit"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.rateLimits.memoryLimitMb')}</FormLabel>
                                        <FormControl>
                                            <Input type="number" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                        </FormControl>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />

                            <FormField
                                control={form.control}
                                name="memoryReservation"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.rateLimits.memoryReservationMb')}</FormLabel>
                                        <FormControl>
                                            <Input type="number" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                        </FormControl>
                                        <FormMessage />
                                        {!readonly && suggestedMemoryMb !== undefined && (
                                            <TooltipProvider>
                                                <Tooltip delayDuration={200}>
                                                    <TooltipTrigger asChild>
                                                        <span
                                                            className="inline-flex cursor-pointer items-center rounded-full border border-blue-300 bg-blue-50 px-2 py-0.5 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-700 dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900"
                                                            onClick={() => form.setValue('memoryReservation', suggestedMemoryMb)}
                                                        >
                                                            ~ {suggestedMemoryMb} MB
                                                        </span>
                                                    </TooltipTrigger>
                                                    <TooltipContent>
                                                        <p>{t('app.rateLimits.suggestionFromUsage')}</p>
                                                    </TooltipContent>
                                                </Tooltip>
                                            </TooltipProvider>
                                        )}
                                    </FormItem>
                                )}
                            />

                            <FormField
                                control={form.control}
                                name="cpuLimit"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.rateLimits.cpuLimitMillicores')}</FormLabel>
                                        <FormControl>
                                            <Input type="number" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                        </FormControl>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />

                            <FormField
                                control={form.control}
                                name="cpuReservation"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.rateLimits.cpuReservationMillicores')}</FormLabel>
                                        <FormControl>
                                            <Input type="number" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                        </FormControl>
                                        <FormMessage />
                                        {!readonly && suggestedCpuMillicores !== undefined && (
                                            <TooltipProvider>
                                                <Tooltip delayDuration={200}>
                                                    <TooltipTrigger asChild>
                                                        <span
                                                            className="inline-flex cursor-pointer items-center rounded-full border border-blue-300 bg-blue-50 px-2 py-0.5 text-xs font-medium text-blue-700 hover:bg-blue-100 dark:border-blue-700 dark:bg-blue-950 dark:text-blue-300 dark:hover:bg-blue-900"
                                                            onClick={() => form.setValue('cpuReservation', suggestedCpuMillicores)}
                                                        >
                                                            ~ {suggestedCpuMillicores} m
                                                        </span>
                                                    </TooltipTrigger>
                                                    <TooltipContent>
                                                        <p>{t('app.rateLimits.suggestionFromUsage')}</p>
                                                    </TooltipContent>
                                                </Tooltip>
                                            </TooltipProvider>
                                        )}
                                    </FormItem>
                                )}
                            />
                        </div>
                    </CardContent>
                    {!readonly && <CardFooter className="gap-4">
                        <SubmitButton>{t('common.save')}</SubmitButton>
                        <p className="text-red-500">{state?.message}</p>
                    </CardFooter>}
                </form>
            </Form >
        </Card >

    </>;
}
