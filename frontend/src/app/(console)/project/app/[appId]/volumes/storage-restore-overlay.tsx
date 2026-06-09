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
import { AppVolume } from "@/shared/model/prisma-compat"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { restoreVolumeFromZip } from "./actions"
import { toast } from "sonner"
import { AppExtendedModel } from "@/shared/model/app-extended.model"
import { VolumeUploadModel, volumeUploadZodModel } from "@/shared/model/volume-upload.model"
import { useT } from "@/i18n"

const accessModes = [
  { label: "ReadWriteOnce", value: "ReadWriteOnce" },
  { label: "ReadWriteMany", value: "ReadWriteMany" },
] as const

export default function StorageRestoreDialog({ children, volume, app }: { children: React.ReactNode; volume: AppVolume; app: AppExtendedModel; }) {

  const t = useT();
  const [isOpen, setIsOpen] = useState<boolean>(false);


  const form = useForm<VolumeUploadModel>({
    resolver: zodResolver(volumeUploadZodModel) as any
  });

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: FormData) =>
    restoreVolumeFromZip(state, payload, volume.id), FormUtils.getInitialFormState<typeof volumeUploadZodModel>());

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success(t('app.storageRestore.uploaded'), {
      });
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof volumeUploadZodModel>(state, form as any);
  }, [state]);

  useEffect(() => {
    form.reset();
  }, [volume, app, children]);

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{t('app.storageRestore.title')}</DialogTitle>
            <DialogDescription>
              {t('app.storageRestore.description')}
            </DialogDescription>
          </DialogHeader>
          <Form {...form}>
            <form action={formAction}>
              <div className="space-y-4">
                <FormField
                  control={form.control}
                  name="file"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.storageRestore.file')}</FormLabel>
                      <FormControl>
                        <Input type="file" {...field} accept="application/gzip" />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <p className="text-red-500">{state.message ?? t('app.storageRestore.dataLossWarning')}</p>
                <SubmitButton>{t('app.storageRestore.restore')}</SubmitButton>
              </div>
            </form>
          </Form >
        </DialogContent>
      </Dialog>
    </>
  )



}
