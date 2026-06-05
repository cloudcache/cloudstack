'use client'

import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useForm, useFieldArray } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Form, FormField, FormItem, FormControl, FormMessage, FormLabel } from "@/components/ui/form";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Trash, Plus } from "lucide-react";
import FormLabelWithQuestion from "@/components/custom/form-label-with-question";
import { useActionState, useTransition } from 'react';
import { saveHealthCheck } from "./actions";
import { useEffect } from "react";
import { toast } from "sonner";
import { SubmitButton } from "@/components/custom/submit-button";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { HealthCheckModel, healthCheckZodModel } from "./health-check.model";
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { useT } from "@/i18n";

export default function HealthCheckSettings({ app, readonly }: { app: AppExtendedModel, readonly: boolean }) {
    const t = useT();

    const defaultHeaders = app.healthCheckHttpHeadersJson
        ? JSON.parse(app.healthCheckHttpHeadersJson)
        : [];

    const isEnabled = !!(app.healthChechHttpGetPath || app.healthCheckTcpPort);
    const probeType = app.healthChechHttpGetPath ? "HTTP" : app.healthCheckTcpPort ? "TCP" : "HTTP";

    const defaultValues: HealthCheckModel = {
        appId: app.id,
        enabled: isEnabled,
        probeType: probeType as "HTTP" | "TCP",
        path: app.healthChechHttpGetPath || undefined,
        httpPort: app.healthCheckHttpPort || undefined,
        scheme: (app.healthCheckHttpScheme as "HTTP" | "HTTPS") || "HTTP",
        periodSeconds: app.healthCheckPeriodSeconds ?? 15,
        timeoutSeconds: app.healthCheckTimeoutSeconds ?? 5,
        failureThreshold: app.healthCheckFailureThreshold ?? 3,
        headers: defaultHeaders,
        tcpPort: app.healthCheckTcpPort || undefined,
    };

    const form = useForm<HealthCheckModel>({
        resolver: zodResolver(healthCheckZodModel) as any,
        defaultValues,
        disabled: readonly,
    });

    const { fields, append, remove } = useFieldArray({
        control: form.control,
        name: "headers"
    });

    const enabled = form.watch("enabled");
    const probeTypeWatch = form.watch("probeType");

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState(
        (state: ServerActionResult<any, any>, payload: HealthCheckModel) => saveHealthCheck(state, payload),
        FormUtils.getInitialFormState<typeof healthCheckZodModel>()
    );

    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('app.health.saved'));
        }
        FormUtils.mapValidationErrorsToForm<typeof healthCheckZodModel>(state, form as any);
    }, [state]);

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('app.health.title')}</CardTitle>
                <CardDescription>
                    {t('app.health.description')}
                </CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                    startTransition(() => formAction(data));
                })()}>
                    <CardContent className="space-y-6">
                        <FormField
                            control={form.control}
                            name="enabled"
                            render={({ field }) => (
                                <FormItem className="flex flex-row items-center justify-between rounded-lg border p-3 shadow-sm">
                                    <div className="space-y-0.5">
                                        <FormLabel>{t('app.health.enable')}</FormLabel>
                                    </div>
                                    <FormControl>
                                        <Switch
                                            checked={field.value}
                                            onCheckedChange={field.onChange}
                                            disabled={readonly}
                                        />
                                    </FormControl>
                                </FormItem>
                            )}
                        />

                        {enabled && (
                            <>
                                <Tabs value={probeTypeWatch} onValueChange={(value) => form.setValue('probeType', value as "HTTP" | "TCP")} className="w-full">
                                    <TabsList className="mb-2">
                                        <TabsTrigger value="HTTP">{t('app.health.httpProbe')}</TabsTrigger>
                                        <TabsTrigger value="TCP">{t('app.health.tcpProbe')}</TabsTrigger>
                                    </TabsList>

                                    <TabsContent value="HTTP" className="space-y-4 mt-4">
                                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                            <FormField
                                                control={form.control}
                                                name="path"
                                                render={({ field }) => (
                                                    <FormItem>
                                                        <FormLabelWithQuestion hint={t('app.health.httpPathHint')}>
                                                            {t('app.health.httpPath')}
                                                        </FormLabelWithQuestion>
                                                        <FormControl>
                                                            <Input placeholder="/healthz" {...field} value={field.value || ''} />
                                                        </FormControl>
                                                        <FormMessage />
                                                    </FormItem>
                                                )}
                                            />
                                            <FormField
                                                control={form.control}
                                                name="httpPort"
                                                render={({ field }) => (
                                                    <FormItem>
                                                        <FormLabelWithQuestion hint={t('app.health.httpPortHint')}>
                                                            {t('app.health.httpPort')}
                                                        </FormLabelWithQuestion>
                                                        <FormControl>
                                                            <Input type="number" placeholder="80" {...field} value={field.value || ''} onChange={e => field.onChange(e.target.value)} />
                                                        </FormControl>
                                                        <FormMessage />
                                                    </FormItem>
                                                )}
                                            />
                                            <FormField
                                                control={form.control}
                                                name="scheme"
                                                render={({ field }) => (
                                                    <FormItem>
                                                        <FormLabelWithQuestion hint={
                                                            <div>
                                                                <p>{t('app.health.schemeHint')}</p>
                                                                <p>{t('app.health.possibleValues')}</p>
                                                                <ul className="list-disc pl-4">
                                                                    <li>{t('app.health.httpSchemeHttp')}</li>
                                                                    <li>{t('app.health.httpSchemeHttps')}</li>
                                                                </ul>
                                                            </div>
                                                        }>
                                                            {t('app.health.httpScheme')}
                                                        </FormLabelWithQuestion>
                                                        <Select onValueChange={field.onChange} defaultValue={field.value} value={field.value}>
                                                            <FormControl>
                                                                <SelectTrigger>
                                                                    <SelectValue placeholder={t('app.health.selectScheme')} />
                                                                </SelectTrigger>
                                                            </FormControl>
                                                            <SelectContent>
                                                                <SelectItem value="HTTP">HTTP</SelectItem>
                                                                <SelectItem value="HTTPS">HTTPS</SelectItem>
                                                            </SelectContent>
                                                        </Select>
                                                        <FormMessage />
                                                    </FormItem>
                                                )}
                                            />
                                        </div>

                                        <div>
                                            <FormLabelWithQuestion hint={
                                                <div>
                                                    <p>{t('app.health.headersHint')}</p>
                                                </div>
                                            }>
                                                {t('app.health.httpHeaders')}
                                            </FormLabelWithQuestion>
                                            <div className="space-y-2 mt-2">
                                                {fields.map((item, index) => (
                                                    <div key={item.id} className="flex gap-2 items-start">
                                                        <FormField
                                                            control={form.control}
                                                            name={`headers.${index}.name`}
                                                            render={({ field }) => (
                                                                <FormItem className="flex-1">
                                                                    {index === 0 && <div className="flex items-center gap-1 mb-1">
                                                                        <FormLabel className="text-xs text-muted-foreground">{t('common.name')}</FormLabel>
                                                                        <FormLabelWithQuestion hint={t('app.health.headerNameHint')}>
                                                                            {''}
                                                                        </FormLabelWithQuestion>
                                                                    </div>}
                                                                    <FormControl>
                                                                        <Input placeholder="X-Custom-Header" {...field} />
                                                                    </FormControl>
                                                                    <FormMessage />
                                                                </FormItem>
                                                            )}
                                                        />
                                                        <FormField
                                                            control={form.control}
                                                            name={`headers.${index}.value`}
                                                            render={({ field }) => (
                                                                <FormItem className="flex-1">
                                                                    {index === 0 && <div className="flex items-center gap-1 mb-1">
                                                                        <FormLabel className="text-xs text-muted-foreground">{t('app.health.value')}</FormLabel>
                                                                        <FormLabelWithQuestion hint={t('app.health.headerValueHint')}>
                                                                            {''}
                                                                        </FormLabelWithQuestion>
                                                                    </div>}
                                                                    <FormControl>
                                                                        <Input placeholder="value" {...field} />
                                                                    </FormControl>
                                                                    <FormMessage />
                                                                </FormItem>
                                                            )}
                                                        />
                                                        <Button
                                                            type="button"
                                                            variant="ghost"
                                                            size="icon"
                                                            disabled={readonly}
                                                            onClick={() => remove(index)}
                                                            className={index === 0 ? 'mt-7' : ''}
                                                        >
                                                            <Trash className="h-4 w-4" />
                                                        </Button>
                                                    </div>
                                                ))}
                                                <Button
                                                    type="button"
                                                    variant="outline"
                                                    size="sm"
                                                    disabled={readonly}
                                                    onClick={() => append({ name: '', value: '' })}
                                                >
                                                    <Plus className="mr-2 h-4 w-4" />
                                                    {t('app.health.addHeader')}
                                                </Button>
                                            </div>
                                        </div>
                                    </TabsContent>

                                    <TabsContent value="TCP" className="space-y-4 mt-4">
                                        <FormField
                                            control={form.control}
                                            name="tcpPort"
                                            render={({ field }) => (
                                                <FormItem>
                                                    <FormLabelWithQuestion hint={t('app.health.tcpPortHint')}>
                                                        {t('app.health.tcpPort')}
                                                    </FormLabelWithQuestion>
                                                    <FormControl>
                                                        <Input type="number" placeholder="3306" {...field} value={field.value || ''} onChange={e => field.onChange(e.target.value)} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        />
                                    </TabsContent>
                                </Tabs>

                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <FormField
                                        control={form.control}
                                        name="periodSeconds"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabelWithQuestion hint={t('app.health.intervalHint')}>
                                                    {t('app.health.interval')}
                                                </FormLabelWithQuestion>
                                                <FormControl>
                                                    <Input type="number" {...field} onChange={e => field.onChange(e.target.value)} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        control={form.control}
                                        name="timeoutSeconds"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabelWithQuestion hint={
                                                    <div>
                                                        <p>{t('app.health.timeoutHint')}</p>
                                                    </div>
                                                }>
                                                    {t('app.health.timeout')}
                                                </FormLabelWithQuestion>
                                                <FormControl>
                                                    <Input type="number" {...field} onChange={e => field.onChange(e.target.value)} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        control={form.control}
                                        name="failureThreshold"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabelWithQuestion hint={t('app.health.failureThresholdHint')}>
                                                    {t('app.health.failureThreshold')}
                                                </FormLabelWithQuestion>
                                                <FormControl>
                                                    <Input type="number" {...field} onChange={e => field.onChange(e.target.value)} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                </div>
                            </>
                        )}
                    </CardContent>
                    <CardFooter>
                        <SubmitButton>{t('common.save')}</SubmitButton>
                    </CardFooter>
                </form>
            </Form>
        </Card>
    );
}
