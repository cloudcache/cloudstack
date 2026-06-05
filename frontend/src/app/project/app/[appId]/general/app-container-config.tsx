'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm, useFieldArray } from "react-hook-form";
import { saveGeneralAppContainerConfig } from "./actions";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { type ReactNode, useEffect } from "react";
import { toast } from "sonner";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { HelpCircle, Plus, Trash2 } from "lucide-react";
import { z } from "zod";
import { appContainerConfigZodModel } from "@/shared/model/app-container-config.model";
import { useT } from "@/i18n";

export type AppContainerConfigInputModel = z.infer<typeof appContainerConfigZodModel>;

function LabelWithHint({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
    const t = useT();

    return (
        <div className="flex items-center gap-1.5">
            <FormLabel className="m-0">{children}</FormLabel>
            {hint && (
                <Tooltip>
                    <TooltipTrigger asChild>
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="h-5 w-5 text-muted-foreground hover:text-foreground"
                        >
                            <HelpCircle className="h-3.5 w-3.5" />
                            <span className="sr-only">{t('common.moreInformation')}</span>
                        </Button>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="max-w-80">
                        <div className="text-sm leading-relaxed">{hint}</div>
                    </TooltipContent>
                </Tooltip>
            )}
        </div>
    );
}

export default function GeneralAppContainerConfig({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {
    const t = useT();
    // Parse containerArgs from JSON string to array
    const initialArgs = app.containerArgs
        ? JSON.parse(app.containerArgs).map((arg: string) => ({ value: arg }))
        : [];

    const form = useForm<AppContainerConfigInputModel>({
        resolver: zodResolver(appContainerConfigZodModel) as any,
        defaultValues: {
            containerCommand: app.containerCommand || '',
            containerArgs: initialArgs,
            securityContextRunAsUser: app.securityContextRunAsUser ?? undefined,
            securityContextRunAsGroup: app.securityContextRunAsGroup ?? undefined,
            securityContextFsGroup: app.securityContextFsGroup ?? undefined,
            securityContextPrivileged: app.securityContextPrivileged ?? false,
        },
        disabled: readonly,
    });

    const { fields, append, remove } = useFieldArray({
        control: form.control,
        name: "containerArgs",
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState(
        (state: any, payload: any) =>
            saveGeneralAppContainerConfig(state, payload, app.projectId, app.id),
        FormUtils.getInitialFormState<typeof appContainerConfigZodModel>() as any
    );

    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('app.container.saved'), {
                description: t('app.source.deployHint'),
            });
        }
        FormUtils.mapValidationErrorsToForm<typeof appContainerConfigZodModel>(state, form as any)
    }, [state]);

    const values = form.watch();

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('app.container.title')}</CardTitle>
                <CardDescription>
                    {t('app.container.description')}
                </CardDescription>
            </CardHeader>
            <Form {...form}>
                <TooltipProvider delayDuration={150}>
                    <form action={(e) => form.handleSubmit((data) => {
                        return startTransition(() => formAction(data));
                    })()}>
                        <CardContent className="space-y-6">
                            <div className="space-y-4">
                                <div className="space-y-1">
                                    <p className="text-sm font-medium">{t('app.container.runtime')}</p>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.container.runtimeDescription')}
                                    </p>
                                </div>

                                <FormField
                                    control={form.control}
                                    name="containerCommand"
                                    render={({ field }) => (
                                        <FormItem>
                                            <LabelWithHint hint={t('app.container.commandHint')}>
                                                {t('app.container.command')}
                                            </LabelWithHint>
                                            <FormControl>
                                                <Input
                                                    placeholder={t('app.container.commandPlaceholder')}
                                                    {...field}
                                                    value={field.value as string | number | readonly string[] | undefined}
                                                />
                                            </FormControl>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />

                                <div className="space-y-3">
                                    <LabelWithHint hint={t('app.container.argumentsHint')}>
                                        {t('app.container.arguments')}
                                    </LabelWithHint>

                                    <div className="space-y-2">
                                        {fields.length === 0 && (
                                            <div className="rounded-md border border-dashed px-3 py-2 text-sm text-muted-foreground">
                                                {t('app.container.noArguments')}
                                            </div>
                                        )}

                                        {fields.map((field, index) => (
                                            <div key={field.id} className="flex items-start gap-2">
                                                <FormField
                                                    control={form.control}
                                                    name={`containerArgs.${index}.value`}
                                                    render={({ field }) => (
                                                        <FormItem className="flex-1">
                                                            <FormControl>
                                                                <Input
                                                                    placeholder={t('app.container.argumentPlaceholder', { index: index + 1 })}
                                                                    {...field}
                                                                />
                                                            </FormControl>
                                                            <FormMessage />
                                                        </FormItem>
                                                    )}
                                                />
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="icon"
                                                    className="mt-0"
                                                    onClick={() => remove(index)}
                                                    disabled={readonly}
                                                >
                                                    <Trash2 className="h-4 w-4" />
                                                </Button>
                                            </div>
                                        ))}
                                    </div>

                                    {!readonly && (
                                        <Button
                                            type="button"
                                            variant="outline"
                                            size="sm"
                                            onClick={() => append({ value: '' })}
                                        >
                                            <Plus className="mr-2 h-4 w-4" />
                                            {t('app.container.addArgument')}
                                        </Button>
                                    )}
                                </div>
                            </div>

                            <Separator />

                            <div className="space-y-4">
                                <div className="space-y-1">
                                    <p className="text-sm font-medium">{t('app.container.securityContext')}</p>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.container.securityDescription')}
                                    </p>
                                </div>

                                <div className="grid gap-4 md:grid-cols-3">
                                    <FormField
                                        control={form.control}
                                        name="securityContextRunAsUser"
                                        render={({ field }) => (
                                            <FormItem>
                                                <LabelWithHint hint={t('app.container.runAsUserHint')}>
                                                    {t('app.container.runAsUser')}
                                                </LabelWithHint>
                                                <FormControl>
                                                    <Input
                                                        type="number"
                                                        placeholder="e.g., 1001"
                                                        {...field}
                                                        value={field.value ?? ''}
                                                        onChange={e => field.onChange(e.target.value === '' ? null : Number(e.target.value))}
                                                    />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        control={form.control}
                                        name="securityContextRunAsGroup"
                                        render={({ field }) => (
                                            <FormItem>
                                                <LabelWithHint hint={t('app.container.runAsGroupHint')}>
                                                    {t('app.container.runAsGroup')}
                                                </LabelWithHint>
                                                <FormControl>
                                                    <Input
                                                        type="number"
                                                        placeholder="e.g., 1001"
                                                        {...field}
                                                        value={field.value ?? ''}
                                                        onChange={e => field.onChange(e.target.value === '' ? null : Number(e.target.value))}
                                                    />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        control={form.control}
                                        name="securityContextFsGroup"
                                        render={({ field }) => (
                                            <FormItem>
                                                <LabelWithHint hint={t('app.container.fsGroupHint')}>
                                                    {t('app.container.fsGroup')}
                                                </LabelWithHint>
                                                <FormControl>
                                                    <Input
                                                        type="number"
                                                        placeholder="e.g., 1001"
                                                        {...field}
                                                        value={field.value ?? ''}
                                                        onChange={e => field.onChange(e.target.value === '' ? null : Number(e.target.value))}
                                                    />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                </div>

                                <FormField
                                    control={form.control}
                                    name="securityContextPrivileged"
                                    render={({ field }) => (
                                        <FormItem className="space-y-3 rounded-md border p-4">
                                            <div className="flex items-start gap-4">
                                                <FormControl>
                                                    <Switch
                                                        checked={field.value ?? false}
                                                        onCheckedChange={field.onChange}
                                                        disabled={readonly}
                                                    />
                                                </FormControl>
                                                <div className="space-y-3 pt-0.5">
                                                    <LabelWithHint
                                                        hint={(
                                                            <>
                                                                <p>
                                                                    {t('app.container.privilegedHint1')}
                                                                </p>
                                                                <p className="mt-2">
                                                                    {t('app.container.privilegedHint2')}
                                                                </p>
                                                            </>
                                                        )}
                                                    >
                                                        {t('app.container.privilegedMode')}
                                                    </LabelWithHint>

                                                    {values.securityContextPrivileged && <Alert className="border-amber-200 bg-amber-50 text-amber-950">
                                                        <AlertDescription>
                                                            {t('app.container.privilegedWarning')}
                                                        </AlertDescription>
                                                    </Alert>}
                                                </div>

                                            </div>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />
                            </div>
                        </CardContent>
                        {!readonly && (
                            <CardFooter className="gap-4">
                                <SubmitButton>{t('common.save')}</SubmitButton>
                                <p className="text-red-500">{state?.message}</p>
                            </CardFooter>
                        )}
                    </form>
                </TooltipProvider>
            </Form>
        </Card>
    );
}
