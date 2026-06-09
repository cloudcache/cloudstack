'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { AppSourceInfoInputModel, appSourceInfoInputZodModel } from "@/shared/model/app-source-info.model";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { saveGeneralAppSourceInfo } from "./actions";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Label } from "@/components/ui/label";
import { useEffect } from "react";
import { App } from "@/shared/model/prisma-compat";
import { toast } from "sonner";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useT } from "@/i18n";

export default function GeneralAppSource({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {
    const t = useT();
    const form = useForm<AppSourceInfoInputModel>({
        resolver: zodResolver(appSourceInfoInputZodModel) as any,
        defaultValues: {
            ...app,
            sourceType: app.sourceType as 'GIT' | 'CONTAINER'
        },
        disabled: readonly,
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: AppSourceInfoInputModel) => saveGeneralAppSourceInfo(state, payload, app.projectId, app.id), FormUtils.getInitialFormState<typeof appSourceInfoInputZodModel>());
    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('app.source.saved'), {
                description: t('app.source.deployHint'),
            });
        }
        FormUtils.mapValidationErrorsToForm<typeof appSourceInfoInputZodModel>(state, form as any)
    }, [state]);

    const sourceTypeField = form.watch();
    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.source.title')}</CardTitle>
                <CardDescription>{t('app.source.description')}</CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                    return startTransition(() => formAction(data));
                })()}>
                    <CardContent className="space-y-4">
                        <div className="hidden">
                            <FormField
                                control={form.control}
                                name="sourceType"
                                render={({ field }) => (
                                    <FormItem>
                                        <FormLabel>{t('app.source.sourceType')}</FormLabel>
                                        <FormControl>
                                            <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                        </FormControl>
                                        <FormMessage />
                                    </FormItem>
                                )}
                            />
                        </div>
                        <Label>{t('app.source.sourceType')}</Label>
                        <Tabs defaultValue="GIT" value={sourceTypeField.sourceType} onValueChange={(val) => {
                            form.setValue('sourceType', val as 'GIT' | 'CONTAINER');
                        }} className="mt-2">
                            <TabsList>
                                {app.appType === 'APP' && <TabsTrigger value="GIT">Git</TabsTrigger>}
                                <TabsTrigger value="CONTAINER">{t('app.source.dockerContainer')}</TabsTrigger>
                            </TabsList>
                            <TabsContent value="GIT" className="space-y-4 mt-4">
                                <FormField
                                    control={form.control}
                                    name="gitUrl"
                                    render={({ field }) => (
                                        <FormItem>
                                            <FormLabel>{t('app.source.gitRepoUrl')}</FormLabel>
                                            <FormControl>
                                                <Input  {...field} value={field.value as string | number | readonly string[] | undefined} />
                                            </FormControl>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />
                                <div className="grid grid-cols-2 gap-4">

                                    <FormField
                                        control={form.control}
                                        name="gitUsername"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.gitUsername')}</FormLabel>
                                                <FormControl>
                                                    <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <FormField
                                        control={form.control}
                                        name="gitToken"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.gitToken')}</FormLabel>
                                                <FormControl>
                                                    <Input type="password" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <FormField
                                        control={form.control}
                                        name="gitBranch"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.gitBranch')}</FormLabel>
                                                <FormControl>
                                                    <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        control={form.control}
                                        name="dockerfilePath"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.dockerfilePath')}</FormLabel>
                                                <FormControl>
                                                    <Input placeholder="./Dockerfile"  {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                </div>


                            </TabsContent>
                            <TabsContent value="CONTAINER" className="space-y-4 mt-4">
                                <FormField
                                    control={form.control}
                                    name="containerImageSource"
                                    render={({ field }) => (
                                        <FormItem>
                                            <FormLabel>{t('app.source.dockerImageName')}</FormLabel>
                                            <FormControl>
                                                <Input   {...field} value={field.value as string | number | readonly string[] | undefined} />
                                            </FormControl>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />
                                <div className="grid grid-cols-2 gap-4">

                                    <FormField
                                        control={form.control}
                                        name="containerRegistryUsername"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.registryUsername')}</FormLabel>
                                                <FormControl>
                                                    <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormDescription>{t('app.source.privateRegistryHint')}</FormDescription>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />

                                    <FormField
                                        control={form.control}
                                        name="containerRegistryPassword"
                                        render={({ field }) => (
                                            <FormItem>
                                                <FormLabel>{t('app.source.registryPassword')}</FormLabel>
                                                <FormControl>
                                                    <Input type="password" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                                </FormControl>
                                                <FormDescription>{t('app.source.privateRegistryHint')}</FormDescription>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                </div>
                            </TabsContent>
                        </Tabs>
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
