'use server'

import { backend } from "@/server/adapter/backend-api.adapter";
import UserRegistrationForm from "./register-from";
import UserLoginForm from "./login-form";
import { getUserSession } from "@/server/utils/action-wrapper.utils";
import { redirect } from "next/navigation";
import { getT } from "@/i18n/server";

interface Props {
    searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}

export default async function AuthPage({ searchParams }: Props) {
    const params = await searchParams;
    const session = await getUserSession();
    const { t } = await getT();
    if (session) {
        redirect('/projects');
    }

    const mode = params.mode; // ?mode=register to explicitly show register form
    const expired = params.expired === '1';
    const signedOut = params.signedOut === '1';

    let registrationOpen = false;
    let firstBoot = false;
    try {
        const status = await backend.auth.registrationStatus();
        registrationOpen = status.enabled;
        firstBoot = status.first_boot;
    } catch {
        // If backend unreachable, show login form
    }

    // Show register form when:
    //   1. First boot (no users) — always show register
    //   2. Registration is open AND user explicitly navigates to ?mode=register
    // Otherwise show login form
    const showRegister = registrationOpen && (firstBoot || mode === 'register');

    return (
        <div className="flex items-center justify-center" style={{ height: '95vh' }}>
            <div className="flex flex-col items-center gap-4">
                {expired && (
                    <p className="rounded-md border border-amber-200 bg-amber-50 px-4 py-2 text-sm text-amber-800">
                        {t("auth.sessionExpired")}
                    </p>
                )}
                {signedOut && (
                    <p className="rounded-md border border-emerald-200 bg-emerald-50 px-4 py-2 text-sm text-emerald-800">
                        {t("auth.signedOut")}
                    </p>
                )}
                {showRegister ? <UserRegistrationForm /> : <UserLoginForm />}
                {registrationOpen && !showRegister && (
                    <p className="text-sm text-muted-foreground">
                        {t("auth.noAccount")}{' '}
                        <a href="/auth?mode=register" className="text-primary underline">{t("auth.register")}</a>
                    </p>
                )}
                {showRegister && !firstBoot && (
                    <p className="text-sm text-muted-foreground">
                        {t("auth.haveAccount")}{' '}
                        <a href="/auth" className="text-primary underline">{t("auth.signIn")}</a>
                    </p>
                )}
            </div>
        </div>
    )
}
