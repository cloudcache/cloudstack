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
import { AppVolumeEditModel, appVolumeEditZodModel } from "@/shared/model/volume-edit.model"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { saveVolume, getShareableVolumes } from "./actions"
import { toast } from "sonner"
import { AppExtendedModel } from "@/shared/model/app-extended.model"
import SelectFormField from "@/components/custom/select-form-field"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Info } from "lucide-react"
import { useT } from "@/i18n"

type ShareableVolume = {
    id: string;
    containerMountPath: string;
    size: number;
    storageClassName: string;
    accessMode: string;
    app: { name: string };
};

export default function SharedStorageEditDialog({ children, app }: {
    children: React.ReactNode;
    app: AppExtendedModel;
}) {

    const t = useT();
    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [shareableVolumes, setShareableVolumes] = useState<ShareableVolume[]>([]);
    const [isLoadingVolumes, setIsLoadingVolumes] = useState(false);

    const form = useForm<AppVolumeEditModel>({
        resolver: zodResolver(appVolumeEditZodModel) as any,
        defaultValues: {
            containerMountPath: '',
            size: 0,
            accessMode: 'ReadWriteMany',
            storageClassName: 'longhorn',
            sharedVolumeId: undefined,
        }
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState<ServerActionResult<any, any>, AppVolumeEditModel>((state, payload) =>
        saveVolume(state, {
            ...payload,
            appId: app.id,
            id: undefined
        } as any), FormUtils.getInitialFormState<typeof appVolumeEditZodModel>() as ServerActionResult<any, any>);

    // Fetch shareable volumes when dialog opens
    useEffect(() => {
        if (isOpen) {
            setIsLoadingVolumes(true);
            getShareableVolumes(app.id).then(result => {
                if (result.status === 'success' && result.data) {
                    const alreadyAddedSharedVolumes = app.appVolumes
                        .filter(v => !!v.sharedVolumeId)
                        .map(v => v.sharedVolumeId);
                    const volumes = result.data as ShareableVolume[];
                    setShareableVolumes(volumes.filter(v => !alreadyAddedSharedVolumes.includes(v.id)));
                } else {
                    setShareableVolumes([]);
                    toast.error(t('app.sharedStorage.fetchFailed'));
                }
                setIsLoadingVolumes(false);
            });
        }
    }, [isOpen, app.id]);

    // Watch selected volume and auto-fill fields
    const watchedSharedVolumeId = form.watch("sharedVolumeId");
    useEffect(() => {
        if (watchedSharedVolumeId) {
            const selectedVolume = shareableVolumes.find(v => v.id === watchedSharedVolumeId);
            if (selectedVolume) {
                form.setValue("size", selectedVolume.size);
                form.setValue("accessMode", selectedVolume.accessMode);
                form.setValue("storageClassName", selectedVolume.storageClassName as 'longhorn' | 'local-path');
            }
        }
    }, [watchedSharedVolumeId, shareableVolumes]);

    useEffect(() => {
        if (state.status === 'success') {
            form.reset();
            toast.success(t('app.sharedStorage.mounted'), {
                description: t('app.common.deployToApply'),
            });
            setIsOpen(false);
        }
        FormUtils.mapValidationErrorsToForm<typeof appVolumeEditZodModel>(state, form as any);
    }, [state]);

    return (
        <>
            <div onClick={() => setIsOpen(true)}>
                {children}
            </div>
            <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
                <DialogContent className="sm:max-w-[425px]">
                    <DialogHeader>
                        <DialogTitle>{t('app.sharedStorage.mountTitle')}</DialogTitle>
                        <DialogDescription>
                            {t('app.sharedStorage.mountDescription')}
                        </DialogDescription>
                    </DialogHeader>
                    <Form {...form}>
                        <form action={(e) => form.handleSubmit((data) => {
                            return startTransition(() => formAction(data));
                        })()}>
                            <div className="space-y-4">
                                {isLoadingVolumes ? (
                                    <div className="text-sm text-muted-foreground">{t('app.sharedStorage.loading')}</div>
                                ) : shareableVolumes.length === 0 ? (
                                    <Alert>
                                        <Info className="h-4 w-4" />
                                        <AlertDescription>
                                            {t('app.sharedStorage.empty')}
                                        </AlertDescription>
                                    </Alert>
                                ) : (
                                    <>
                                        <SelectFormField
                                            form={form as any}
                                            name="sharedVolumeId"
                                            label={t('app.sharedStorage.selectVolume')}
                                            values={shareableVolumes.map(v => [
                                                v.id,
                                                `${v.app.name} - ${v.containerMountPath} (${v.size}MB)`
                                            ])}
                                            placeholder={t('app.sharedStorage.selectPlaceholder')}
                                        />

                                        <FormField
                                            control={form.control}
                                            name="containerMountPath"
                                            render={({ field }) => (
                                                <FormItem>
                                                    <FormLabel>{t('app.sharedStorage.mountPath')}</FormLabel>
                                                    <FormControl>
                                                        <Input placeholder="ex. /shared-data" {...field} />
                                                    </FormControl>
                                                    <FormMessage />
                                                </FormItem>
                                            )}
                                        />

                                        <div className="text-sm text-muted-foreground space-y-1">
                                            <p><strong>{t('app.storage.size')}:</strong> {form.watch("size")} MB ({t('app.sharedStorage.inherited')})</p>
                                            <p><strong>{t('app.storage.storageClass')}:</strong> {form.watch("storageClassName")} ({t('app.sharedStorage.inherited')})</p>
                                        </div>
                                    </>
                                )}

                                <p className="text-red-500">{state.message}</p>
                                {shareableVolumes.length > 0 && <SubmitButton>{t('app.sharedStorage.mountButton')}</SubmitButton>}
                            </div>
                        </form>
                    </Form >
                </DialogContent>
            </Dialog>
        </>
    )
}
