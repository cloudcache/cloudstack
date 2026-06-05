'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { saveEnvVariables } from "./actions";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { useEffect } from "react";
import { toast } from "sonner";
import { AppEnvVariablesModel, appEnvVariablesZodModel } from "@/shared/model/env-edit.model";
import { Textarea } from "@/components/ui/textarea";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useT } from "@/i18n";


export default function EnvEdit({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {
    const t = useT();
    const form = useForm<AppEnvVariablesModel>({
        resolver: zodResolver(appEnvVariablesZodModel) as any,
        defaultValues: app,
        disabled: readonly,
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: AppEnvVariablesModel) => saveEnvVariables(state, payload, app.id), FormUtils.getInitialFormState<typeof appEnvVariablesZodModel>());
    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('app.env.saved'), {
                description: t('app.common.deployToApply'),
            });
        }
        FormUtils.mapValidationErrorsToForm<typeof appEnvVariablesZodModel>(state, form as any);
    }, [state]);

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.env.title')}</CardTitle>
                <CardDescription>
                    {t('app.env.description')}
                    {app.appType !== 'APP' && <div className="text-sm text-red-500 pt-2">{t('app.env.databaseWarning')}</div>}

                </CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                    return startTransition(() => formAction(data));
                })()}>
                    <CardContent className="space-y-4">
                        <FormField
                            control={form.control}
                            name="envVars"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('app.env.variables')}</FormLabel>
                                    <FormControl>
                                        <Textarea className="h-96" placeholder="NAME=VALUE..." {...field} value={field.value} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                    </CardContent>
                    {!readonly && <CardFooter>
                        <SubmitButton>{t('common.save')}</SubmitButton>
                    </CardFooter>}
                </form>
            </Form >
        </Card >
    </>;
}
