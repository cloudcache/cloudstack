'use client'

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
import { useActionState } from 'react'
import { useEffect, useTransition } from "react";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { SubmitButton } from "@/components/custom/submit-button";
import { AuthFormInputSchema, authFormInputSchemaZod, RegisterFormInputSchema, registgerFormInputSchemaZod } from "@/shared/model/auth-form"
import { registerUser } from "./actions"
import { signIn } from "next-auth/react";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { redirect } from "next/navigation"
import FormLabelWithQuestion from "@/components/custom/form-label-with-question"
import { toast } from "sonner"
import { useT } from "@/i18n"

export default function UserRegistrationForm() {
    const t = useT();
    const form = useForm<RegisterFormInputSchema>({
        resolver: zodResolver(registgerFormInputSchemaZod) as any,
        defaultValues: { email: '', password: '', qsHostname: '' },
    });

    const [, startTransition] = useTransition();
    const [state, formAction] = useActionState(registerUser, FormUtils.getInitialFormState<typeof registgerFormInputSchemaZod>());

    useEffect(() => {
        if (state.status === 'success') {
            toast.success(state.message ?? t("auth.registrationSuccess"));
            // Redirect to login form after successful registration
            window.location.href = '/auth';
        }
    }, [state]);

    return (
        <Card className="w-[350px] mx-auto">
            <CardHeader>
                <CardTitle>{t("auth.registration")}</CardTitle>
                <CardDescription>{t("auth.registrationSubtitle")}</CardDescription>
            </CardHeader>
            <Form {...form}>
                <form action={() => form.handleSubmit((data) => startTransition(() => formAction(data)))()}
                    className="space-y-8">
                    <CardContent className="space-y-4">
                        <FormField
                            control={form.control}
                            name="email"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t("auth.email")}</FormLabel>
                                    <FormControl>
                                        <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />

                        <FormField
                            control={form.control}
                            name="password"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabel>{t("auth.password")}</FormLabel>
                                    <FormControl>
                                        <Input type="password"  {...field} value={field.value as string | number | readonly string[] | undefined} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />

                        <FormField
                            control={form.control}
                            name="qsHostname"
                            render={({ field }) => (
                                <FormItem>
                                    <FormLabelWithQuestion hint={t("auth.domainHint")}>
                                        QuickStack Domain (optional)
                                    </FormLabelWithQuestion>
                                    <FormControl>
                                        <Input {...field} value={field.value as string | number | readonly string[] | undefined} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                        <p className="text-red-500">{state?.message}</p>
                    </CardContent>
                    <CardFooter>
                        <SubmitButton className="w-full">{t("auth.register")}</SubmitButton>
                    </CardFooter>
                </form>
            </Form>
        </Card >
    )
}
