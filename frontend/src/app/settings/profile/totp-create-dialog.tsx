'use client';

import { SubmitButton } from "@/components/custom/submit-button";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useActionState, useTransition } from 'react';
import { ServerActionResult } from "@/shared/model/server-action-error-return.model";
import { Input } from "@/components/ui/input";
import { useEffect } from "react";
import { toast } from "sonner";
import { createNewTotpToken, verifyTotpToken } from "./actions";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import React from "react";
import { TotpModel, totpZodModel } from "@/shared/model/totp.model";
import { Toast } from "@/frontend/utils/toast.utils";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { useT } from "@/i18n";

export default function TotpCreateDialog({
    children
}: {
    children: React.ReactNode;
}) {
    const t = useT();
    const [isOpen, setIsOpen] = React.useState(false);
    const [totpQrCode, setTotpQrCode] = React.useState<string | null>(null);

    const form = useForm<TotpModel>({
        resolver: zodResolver(totpZodModel) as any
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: TotpModel) =>
        verifyTotpToken(state, payload), FormUtils.getInitialFormState<typeof totpZodModel>());

    useEffect(() => {
        if (state.status === 'success') {
            toast.success(t('profile.totp.updated'));
            form.setValue('totp', '');
            form.clearErrors();
            setIsOpen(false);
        }
        FormUtils.mapValidationErrorsToForm<typeof totpZodModel>(state, form as any)
    }, [state]);

    const createTotpToken = async () => {
        setIsOpen(true);
        const response = await Toast.fromAction(() => createNewTotpToken());
        if (response.status === 'success') {
            const qrCode = response.data;
            setTotpQrCode(qrCode);
        }
    };

    return <>
        <div onClick={() => createTotpToken()}>
            {children}
        </div>
        <Dialog open={isOpen} onOpenChange={(isO) => setIsOpen(isO)}>
            <DialogContent className="sm:max-w-[425px]">
                <DialogHeader>
                    <DialogTitle>{t('profile.totp.enable')}</DialogTitle>
                    <DialogDescription>
                        {t('profile.totp.description')}
                    </DialogDescription>
                </DialogHeader>
                <div className="space-y-4">
                    {!totpQrCode && <div className="rounded-lg bg-slate-50 py-24"><FullLoadingSpinner /></div>}
                    {totpQrCode && <><img className="mx-auto my-0" src={totpQrCode} /></>}
                    <Form {...form}>
                        <form action={(e) => form.handleSubmit((data) => {
                            return startTransition(() => formAction(data));
                        })()}>
                            <div className="space-y-4">
                                <FormField
                                    control={form.control}
                                    name="totp"
                                    render={({ field }) => (
                                        <FormItem>
                                            <FormLabel>{t('profile.totp.token')}</FormLabel>
                                            <FormControl>
                                                <Input {...field} />
                                            </FormControl>
                                            <FormMessage />
                                        </FormItem>
                                    )}
                                />

                                <p className="text-red-500">{state?.message}</p>
                            </div>
                            <DialogFooter>
                                <SubmitButton>{t('profile.totp.verify')}</SubmitButton>
                            </DialogFooter>
                        </form>
                    </Form >
                </div>
            </DialogContent>
        </Dialog>


    </>;
}
