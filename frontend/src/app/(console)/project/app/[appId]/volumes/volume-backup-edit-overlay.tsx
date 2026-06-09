'use client'

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import {
  Form,
  FormControl,
  FormDescription,
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
import { AppVolume, S3Target, VolumeBackup } from "@/shared/model/prisma-compat"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { saveBackupVolume } from "./actions"
import { toast } from "sonner"
import { VolumeBackupEditModel, volumeBackupEditZodModel } from "@/shared/model/backup-volume-edit.model"
import SelectFormField from "@/components/custom/select-form-field"
import Link from "next/link"
import { Checkbox } from "@/components/ui/checkbox"
import { AppExtendedModel } from "@/shared/model/app-extended.model"
import { useT } from "@/i18n"

export default function VolumeBackupEditDialog({
  children,
  volumeBackup,
  s3Targets,
  volumes,
  app
}: {
  children: React.ReactNode;
  volumeBackup?: VolumeBackup;
  s3Targets: S3Target[];
  volumes: AppVolume[];
  app: AppExtendedModel;
}) {

  const t = useT();
  const [isOpen, setIsOpen] = useState<boolean>(false);

  const isDatabaseApp = app.appType !== 'APP';
  const isDatabaseBackupSupported = [
    'MONGODB',
    //'MYSQL',
    'MARIADB',
    'POSTGRES'
  ].includes(app.appType);

  const form = useForm<VolumeBackupEditModel>({
    resolver: zodResolver(volumeBackupEditZodModel) as any,
    defaultValues: {
      ...volumeBackup,
      retention: volumeBackup?.retention || 5,
      targetId: volumeBackup?.targetId || (s3Targets.length === 1 ? s3Targets[0].id : undefined),
      volumeId: volumeBackup?.volumeId || (volumes.length === 1 ? volumes[0].id : undefined),
      useDatabaseBackup: volumeBackup?.useDatabaseBackup ?? (isDatabaseApp && isDatabaseBackupSupported),
    }
  });

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState<ServerActionResult<any, any>, VolumeBackupEditModel>((state,
    payload) =>
    saveBackupVolume(state, {
      ...payload,
      appId: app.id,
    } as any), FormUtils.getInitialFormState<typeof volumeBackupEditZodModel>() as ServerActionResult<any, any>);

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success(t('app.backups.saved'), {
        description: t('app.backups.savedDescription'),
      });
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof volumeBackupEditZodModel>(state, form as any);
  }, [state]);

  useEffect(() => {
    form.reset(volumeBackup);
  }, [volumeBackup, volumes, s3Targets]);

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{t('app.backups.editTitle')}</DialogTitle>
            <DialogDescription>
              {t('app.backups.editDescription')}
            </DialogDescription>
          </DialogHeader>
          <Form {...form}>
            <form action={(e) => form.handleSubmit((data) => {
              return startTransition(() => formAction(data));
            }, console.error)()}>
              <div className="space-y-4">
                <FormField
                  control={form.control}
                  name="cron"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.backups.cronExpression')}</FormLabel>
                      <FormControl>
                        <Input placeholder="5 4 * * *" {...field} />
                      </FormControl>
                      <FormDescription>
                        {t('app.backups.cronHelp')} <a href="https://crontab.guru/" target="_blank" className="underline">crontab.guru</a>.
                      </FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="retention"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.backups.retention')}</FormLabel>
                      <FormControl>
                        <Input type="number" placeholder="5" {...field} />
                      </FormControl>
                      <FormDescription>
                        {t('app.backups.retentionHelp')}
                      </FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <SelectFormField
                  form={form as any}
                  name="volumeId"
                  label={t('app.backups.volumeToBackup')}
                  values={volumes.map((volume) =>
                    [volume.id, `${volume.containerMountPath}`])}
                />

                <SelectFormField
                  form={form as any}
                  name="targetId"
                  label={t('app.backups.backupLocation')}
                  formDescription={<>
                    {t('app.backups.s3TargetsHelp')} <span className="underline"><Link href="/settings/s3-targets">{t('common.here')}</Link></span>.
                  </>}
                  values={s3Targets.map((target) =>
                    [target.id, `${target.name}`])}
                />

                {isDatabaseApp && (
                  <FormField
                    control={form.control as any}
                    name="useDatabaseBackup"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                        <FormControl>
                          <Checkbox
                            checked={field.value}
                            onCheckedChange={field.onChange}
                            disabled={!isDatabaseBackupSupported}
                          />
                        </FormControl>
                        <div className="space-y-1 leading-none">
                          <FormLabel>
                            {t('app.backups.useDatabaseBackup')}
                          </FormLabel>
                          <FormDescription>
                            {isDatabaseBackupSupported
                              ? t('app.backups.databaseBackupSupported', { type: app.appType.toLocaleLowerCase() })
                              : t('app.backups.databaseBackupUnsupported', { type: app.appType.toLocaleLowerCase() })}
                          </FormDescription>
                        </div>
                      </FormItem>
                    )}
                  />
                )}

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
