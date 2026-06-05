'use client'

import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
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
import { S3Target, User } from "@/shared/model/prisma-compat"
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { toast } from "sonner"
import { ScrollArea } from "@/components/ui/scroll-area"
import { UserEditModel, userEditZodModel } from "@/shared/model/user-edit.model"
import { UserExtended } from "@/shared/model/user-extended.model"
import { saveUser } from "./actions"
import SelectFormField from "@/components/custom/select-form-field"
import { UserGroupExtended } from "@/shared/model/sim-session.model"
import { useT } from "@/i18n"


export default function UserEditOverlay({ children, user, userGroups }: {
  children: React.ReactNode;
  userGroups: UserGroupExtended[];
  user?: UserExtended;
}) {

  const t = useT();
  const [isOpen, setIsOpen] = useState<boolean>(false);


  const form = useForm<UserEditModel>({
    resolver: zodResolver(userEditZodModel) as any,
    defaultValues: user
  });

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState((state: ServerActionResult<any, any>,
    payload: UserEditModel) =>
    saveUser(state, {
      ...payload,
      id: user?.id
    }), FormUtils.getInitialFormState<typeof userEditZodModel>());

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success(t('users.saved'));
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof userEditZodModel>(state, form as any);
  }, [state]);

  useEffect(() => {
    if (user) {
      form.reset(user);
    }
  }, [user]);

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(false)}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>{user?.id ? t('users.editTitle') : t('users.createTitle')}</DialogTitle>
          </DialogHeader>
          <ScrollArea className="max-h-[70vh]">
            <div className="px-2">
              <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                  return startTransition(() => formAction(data));
                })()}>
                  <div className="space-y-4">
                    <FormField
                      control={form.control}
                      name="username"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t('common.username')}</FormLabel>
                          <FormControl>
                            <Input placeholder="" {...field} />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <FormField
                      control={form.control}
                      name="email"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t('common.email')}</FormLabel>
                          <FormControl>
                            <Input placeholder="" {...field} />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <SelectFormField
                      form={form as any}
                      name="userGroupId"
                      label={t('users.group')}
                      formDescription={<>
                        {t('users.groupDescription')}
                      </>}
                      values={userGroups.map((group) =>
                        [group.id, `${group.name}`])}
                    />

                    <FormField
                      control={form.control}
                      name="newPassword"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t('profile.password.new')} {user?.id && <>({t('common.optional')})</>}</FormLabel>
                          <FormControl>
                            <Input type="password" {...field} />
                          </FormControl>
                          <FormDescription>
                            {user?.id && <>{t('users.keepOldPassword')}</>}
                          </FormDescription>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <p className="text-red-500">{state.message}</p>
                    <SubmitButton>{t('common.save')}</SubmitButton>
                  </div>
                </form>
              </Form >
            </div>
          </ScrollArea>
        </DialogContent>
      </Dialog>
    </>
  )



}
