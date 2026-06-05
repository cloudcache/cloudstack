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
import {
  Command,
  CommandGroup,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Input } from "@/components/ui/input"
import { cn } from "@/frontend/utils/utils"
import { Button } from "@/components/ui/button"
import { Check, ChevronsUpDown } from "lucide-react"
import { zodResolver } from "@hookform/resolvers/zod"
import { useForm } from "react-hook-form"
import { useActionState, useTransition } from 'react'
import { useEffect, useState } from "react";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { SubmitButton } from "@/components/custom/submit-button";
import { AppVolume } from "@/shared/model/prisma-compat"
import { AppVolumeEditModel, appVolumeEditZodModel } from "@/shared/model/volume-edit.model"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { saveVolume } from "./actions"
import { toast } from "sonner"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { QuestionMarkCircledIcon } from "@radix-ui/react-icons"
import { AppExtendedModel } from "@/shared/model/app-extended.model"
import { NodeInfoModel } from "@/shared/model/node-info.model"
import CheckboxFormField from "@/components/custom/checkbox-form-field"
import { useT } from "@/i18n"

const accessModes = [
  { label: "ReadWriteOnce", value: "ReadWriteOnce" },
  { label: "ReadWriteMany", value: "ReadWriteMany" },
] as const

const storageClasses = [
  { label: "Longhorn (Default)", value: "longhorn", descriptionKey: "app.storage.longhornDescription" },
  { label: "Local Path", value: "local-path", descriptionKey: "app.storage.localPathDescription" }
] as const

export default function StorageEditDialog({ children, volume, app, nodesInfo }: {
  children: React.ReactNode;
  volume?: AppVolume;
  app: AppExtendedModel;
  nodesInfo: NodeInfoModel[];
}) {
  const t = useT();

  const [isOpen, setIsOpen] = useState<boolean>(false);

  const form = useForm<AppVolumeEditModel>({
    resolver: zodResolver(appVolumeEditZodModel) as any,
    defaultValues: {
      containerMountPath: volume?.containerMountPath ?? '',
      size: volume?.size ?? 0,
      accessMode: volume?.accessMode ?? (app.replicas > 1 ? "ReadWriteMany" : "ReadWriteOnce"),
      storageClassName: (volume?.storageClassName ?? "longhorn") as 'longhorn' | 'local-path',
      shareWithOtherApps: volume?.shareWithOtherApps ?? false,
      sharedVolumeId: volume?.sharedVolumeId ?? undefined,
    }
  });

  // Watch accessMode to conditionally show shareWithOtherApps checkbox
  const watchedAccessMode = form.watch("accessMode");
  const watchedStorageClassName = form.watch("storageClassName");
  const canBeShared = (!!volume ? volume.accessMode : watchedAccessMode === "ReadWriteMany") &&
    watchedStorageClassName !== "local-path" &&
    !volume?.sharedVolumeId;

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState<ServerActionResult<any, any>, AppVolumeEditModel>((state, payload) =>
    saveVolume(state, {
      ...payload,
      appId: app.id,
      id: volume?.id
    } as any), FormUtils.getInitialFormState<typeof appVolumeEditZodModel>() as ServerActionResult<any, any>);

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success(t('app.storage.volumeSaved'), {
        description: t('app.source.deployHint'),
      });
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof appVolumeEditZodModel>(state, form as any);
  }, [state]);

  useEffect(() => {
    form.reset({
      ...volume,
      accessMode: volume?.accessMode ?? (app.replicas > 1 ? "ReadWriteMany" : "ReadWriteOnce"),
      storageClassName: (volume?.storageClassName ?? "longhorn") as 'longhorn' | 'local-path',
      shareWithOtherApps: volume?.shareWithOtherApps ?? false,
      sharedVolumeId: volume?.sharedVolumeId ?? undefined,
    });
  }, [volume]);

  const values = form.watch();

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{t('app.storage.editVolume')}</DialogTitle>
            <DialogDescription>
              {t('app.storage.editDescription')}
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
                      <FormLabel>{t('app.storage.mountPathContainer')}</FormLabel>
                      <FormControl>
                        <Input placeholder="ex. /data" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="size"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t('app.storage.sizeMb')}</FormLabel>
                      <FormControl>
                        <Input type="number" placeholder="ex. 20" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                {volume && volume.size !== values.size && volume.shareWithOtherApps && <>
                  <p className="text-sm text-yellow-600">
                    {t('app.storage.sharedResizeWarning')}
                  </p>
                </>}

                <FormField
                  control={form.control}
                  name="accessMode"
                  disabled={!!volume}
                  render={({ field }) => (
                    <FormItem className="flex flex-col">
                      <FormLabel className="flex gap-2">
                        <div>{t('app.storage.accessMode')}</div>
                        <div className="self-center">
                          <TooltipProvider>
                            <Tooltip>
                              <TooltipTrigger asChild><QuestionMarkCircledIcon /></TooltipTrigger>
                              <TooltipContent>
                                <p className="max-w-[350px]">
                                  {t('app.storage.accessModeHint1')}<br /><br />
                                  {t('app.storage.accessModeHint2')}<br /><br />
                                  {t('app.storage.accessModeHint3')}
                                </p>
                              </TooltipContent>
                            </Tooltip>
                          </TooltipProvider>
                        </div>
                      </FormLabel>
                      <Popover>
                        <PopoverTrigger asChild>
                          <FormControl>
                            <Button
                              variant="outline"
                              role="combobox"
                              disabled={!!volume}
                              className={cn(
                                "w-[200px] justify-between",
                                !field.value && "text-muted-foreground"
                              )}
                            >
                              {field.value
                                ? accessModes.find(
                                  (accessMode) => accessMode.value === field.value
                                )?.label
                                : t('app.storage.selectAccessMode')}
                              <ChevronsUpDown className="opacity-50" />
                            </Button>
                          </FormControl>
                        </PopoverTrigger>
                        <PopoverContent className="w-[200px] p-0">
                          <Command>
                            <CommandList>
                              <CommandGroup>
                                {accessModes.map((accessMode) => (
                                  <CommandItem
                                    value={accessMode.label}
                                    key={accessMode.value}
                                    onSelect={() => {
                                      form.setValue("accessMode", accessMode.value)
                                    }}
                                  >
                                    {accessMode.label}
                                    <Check
                                      className={cn(
                                        "ml-auto",
                                        accessMode.value === field.value
                                          ? "opacity-100"
                                          : "opacity-0"
                                      )}
                                    />
                                  </CommandItem>
                                ))}
                              </CommandGroup>
                            </CommandList>
                          </Command>
                        </PopoverContent>
                      </Popover>
                      <FormDescription>
                        {t('app.storage.cannotChangeAfterCreation')}
                      </FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
                {nodesInfo.length === 1 &&
                  <FormField
                    control={form.control}
                    name="storageClassName"
                    render={({ field }) => (
                      <FormItem className="flex flex-col">
                        <FormLabel className="flex gap-2">
                          <div>{t('app.storage.storageClass')}</div>
                          <div className="self-center">
                            <TooltipProvider>
                              <Tooltip>
                                <TooltipTrigger asChild><QuestionMarkCircledIcon /></TooltipTrigger>
                                <TooltipContent>
                                  <p className="max-w-[350px]">
                                    {t('app.storage.storageClassHint1')}<br /><br />
                                    <b>Longhorn</b> {t('app.storage.longhornHint')}<br />
                                    <b>Local Path</b> {t('app.storage.localPathHint')}
                                  </p>
                                </TooltipContent>
                              </Tooltip>
                            </TooltipProvider>
                          </div>
                        </FormLabel>
                        <Popover>
                          <PopoverTrigger asChild>
                            <FormControl>
                              <Button
                                variant="outline"
                                role="combobox"
                                className={cn(
                                  "w-full justify-between",
                                  !field.value && "text-muted-foreground"
                                )}
                                disabled={!!volume}
                              >
                                {field.value
                                  ? storageClasses.find(
                                    (storageClass) => storageClass.value === field.value
                                  )?.label
                                  : t('app.storage.selectStorageClass')}
                                <ChevronsUpDown className="opacity-50" />
                              </Button>
                            </FormControl>
                          </PopoverTrigger>
                          <PopoverContent className="max-w-[280px] p-0">
                            <Command>
                              <CommandList>
                                <CommandGroup>
                                  {storageClasses.map((storageClass) => (
                                    <CommandItem
                                      value={storageClass.label}
                                      key={storageClass.value}
                                      onSelect={() => {
                                        form.setValue("storageClassName", storageClass.value);
                                      }}
                                    >
                                      <div className="flex flex-col gap-1">
                                        <span>{storageClass.label}</span>
                                        <span className="text-xs text-muted-foreground">{t(storageClass.descriptionKey)}</span>
                                      </div>
                                      <Check
                                        className={cn(
                                          "ml-auto",
                                          storageClass.value === field.value
                                            ? "opacity-100"
                                            : "opacity-0"
                                        )}
                                      />
                                    </CommandItem>
                                  ))}
                                </CommandGroup>
                              </CommandList>
                            </Command>
                          </PopoverContent>
                        </Popover>
                        <FormDescription>
                          {t('app.storage.cannotChangeAfterCreation')}
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />}
                {canBeShared && (
                  <CheckboxFormField
                    form={form}
                    name="shareWithOtherApps"
                    label={t('app.storage.allowOtherAppsAttach')}
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
