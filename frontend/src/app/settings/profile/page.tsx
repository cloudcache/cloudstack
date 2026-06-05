'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import ProfilePasswordChange from "./profile-password-change";
import ToTpSettings from "./totp-settings";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import { backend } from "@/server/adapter/backend-api.adapter";
import { getT } from "@/i18n/server";

export default async function ProjectPage() {

    await getAuthUserSession();
    const token = await getBackendToken();
    const { t } = await getT();
    let me: { totp_enabled?: boolean } = {};
    try {
        me = (await backend.auth.me(token)) as { totp_enabled?: boolean };
    } catch (e) {
        console.error('[Profile] failed to load user info:', e);
    }
    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={t("settings.profile.title")}
                subtitle={`View or edit your Profile information and configure your authentication.`}>
            </PageTitle>
            <BreadcrumbSetter items={[
                { name: t("nav.settings"), url: "/settings/profile" },
                { name: t("settings.profile.title") },
            ]} />
            <ProfilePasswordChange />
            <ToTpSettings totpEnabled={me.totp_enabled ?? false} />
        </div>
    )
}
