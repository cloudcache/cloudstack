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
import { AppFileMount } from "@/shared/model/prisma-compat"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { saveFileMount } from "./actions"
import { toast } from "sonner"
import { AppExtendedModel } from "@/shared/model/app-extended.model"
import { FileMountEditModel, fileMountEditZodModel } from "@/shared/model/file-mount-edit.model"
import { Textarea } from "@/components/ui/textarea"
import { useT } from "@/i18n"

export default function FileMountEditDialog({ children, fileMount, app }: { children: React.ReactNode; fileMount?: AppFileMount; app: AppExtendedModel; }) {

  const t = useT();
  const [isOpen, setIsOpen] = useState<boolean>(false);


  const form = useForm<FileMountEditModel>({
    resolver: zodResolver(fileMountEditZodModel) as any,
    defaultValues: {
      ...fileMount,
    }
  });

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState((state: ServerActionResult<any, any>, payload: FileMountEditModel) =>
    saveFileMount(state, {
      ...payload,
      appId: app.id,
      id: fileMount?.id
    }), FormUtils.getInitialFormState<typeof fileMountEditZodModel>());

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success(t('app.fileMount.saved'), {
        description: t('app.common.deployToApply'),
      });
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof fileMountEditZodModel>(state, form as any);
  }, [state]);

  useEffect(() => {
    form.reset(fileMount);
  }, [fileMount]);

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{t('app.fileMount.editTitle')}</DialogTitle>
            <DialogDescription>
              {t('app.fileMount.editDescription')}
            </DialogDescription>
          </DialogHeader>
          <Form {...form}>
            <form action={(e) => form.handleSubmit((data) => {
              return startTransition(() => formAction(data));
            })()}>
              <div className="space-y-4">
                <FormField
                  control={form.control}
                  name="containerMountPath"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.fileMount.mountPath')}</FormLabel>
                      <FormControl>
                        <Input placeholder="ex. /data/my-config.txt" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="content"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.fileMount.content')}</FormLabel>
                      <FormControl>
                        <Textarea rows={10} placeholder={t('app.fileMount.contentPlaceholder')} {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

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
