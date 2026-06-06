'use client'

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
import { useState } from "react";
import { AuthFormInputSchema, authFormInputSchemaZod } from "@/shared/model/auth-form"
import { signIn } from "next-auth/react";
import LoadingSpinner from "@/components/ui/loading-spinner"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { useT } from "@/i18n";

export default function UserLoginForm() {
    const t = useT();
    const form = useForm<AuthFormInputSchema>({
        resolver: zodResolver(authFormInputSchemaZod) as any,
        defaultValues: { email: '', password: '' },
    });

    const [errorMessages, setErrorMessages] = useState<string | undefined>(undefined);
    const [loading, setLoading] = useState<boolean>(false);

    function redirectToProjects() {
        const currentUrl = window.location.href
        const url = new URL(currentUrl)
        url.pathname = '/'
        url.search = ''
        window.open(url.toString(), '_self')
    }

    const login = async (data: AuthFormInputSchema) => {
        setLoading(true);
        setErrorMessages(undefined);
        try {
            const result = await signIn("credentials", {
                username: data.email,
                password: data.password,
                redirect: false,
            });
            if (result?.error) {
                // Surface email-verification errors with a CTA instead of raw message
                if (result.error === 'EMAIL_NOT_VERIFIED' || result.error.includes('EMAIL_NOT_VERIFIED')) {
                    setErrorMessages('请先在邮箱里完成验证 — 没收到邮件？');
                } else {
                    setErrorMessages(result.error);
                }
            } else {
                redirectToProjects();
            }
        } catch (e) {
            console.error(e);
            setErrorMessages((e as any).message ?? "Login failed");
        } finally {
            setLoading(false);
        }
    }

    return (
        <Card className="w-[350px] mx-auto">
            <CardHeader>
                <CardTitle>{t("auth.signIn")}</CardTitle>
                <CardDescription>{t("auth.signInSubtitle")}</CardDescription>
            </CardHeader>
            <Form {...form}>
                <form onSubmit={async (e) => {
                    e.preventDefault();
                    return form.handleSubmit(async (data) => {
                        await login(data);
                    })();
                }} className="space-y-8">

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
                                        <Input type="password" {...field} value={field.value as string | number | readonly string[] | undefined} />
                                    </FormControl>
                                    <FormMessage />
                                </FormItem>
                            )}
                        />
                    </CardContent>
                    <CardFooter>
                        <p className="text-red-500">{errorMessages}</p>
                        <Button type="submit" className="w-full" disabled={loading}>{loading ? <LoadingSpinner></LoadingSpinner> : t("auth.login")}</Button>
                    </CardFooter>
                </form>
            </Form>
        </Card>
    )
}
