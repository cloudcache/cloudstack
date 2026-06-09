'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { Input } from "@/components/ui/input";
import { useEffect } from "react";
import { toast } from "sonner";
import { ProfilePasswordChangeModel, profilePasswordChangeZodModel } from "@/shared/model/update-password.model";
import { changePassword } from "./actions";
import { useT } from "@/i18n";

export default function ProfilePasswordChange() {
    const t = useT();
    const form = useForm<ProfilePasswordChangeModel>({
        resolver: zodResolver(profilePasswordChangeZodModel) as any
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: ProfilePasswordChangeModel) =>
        changePassword(state, payload), FormUtils.getInitialFormState<typeof profilePasswordChangeZodModel>());

    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('profile.password.updated'));
            form.setValue('oldPassword', '');
            form.setValue('newPassword', '');
            form.setValue('confirmNewPassword', '');
            form.clearErrors();
        }
        FormUtils.mapValidationErrorsToForm<typeof profilePasswordChangeZodModel>(state, form as any)
    }, [state]);

    const sourceTypeField = form.watch();
    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('profile.password.title')}</CardTitle>
                <CardDescription>{t('profile.password.description')}</CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                    return startTransition(() => formAction(data));
                })()}>
                    <CardContent className="space-y-4">
                        <FormField
                            control={form.control}
                            name="oldPassword"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('profile.password.current')}</FormLabel>
                                    <FormControl>
                                        <Input type="password" {...field} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                        <FormField
                            control={form.control}
                            name="newPassword"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('profile.password.new')}</FormLabel>
                                    <FormControl>
                                        <Input type="password" {...field} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                        <FormField
                            control={form.control}
                            name="confirmNewPassword"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t('profile.password.confirmNew')}</FormLabel>
                                    <FormControl>
                                        <Input type="password" {...field} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                    </CardContent>
                    <CardFooter className="gap-4">
                        <SubmitButton>{t('profile.password.change')}</SubmitButton>
                        <p className="text-red-500">{state?.message}</p>
                    </CardFooter>
                </form>
            </Form >
        </Card >

    </>;
}
