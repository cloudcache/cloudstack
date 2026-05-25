'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import ProfilePasswordChange from "./profile-password-change";
import ToTpSettings from "./totp-settings";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import { backend } from "@/server/adapter/backend-api.adapter";

export default async function ProjectPage() {

    await getAuthUserSession();
    const token = await getBackendToken();
    const me = (await backend.auth.me(token)) as { totp_enabled?: boolean };
    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={'Profile'}
                subtitle={`View or edit your Profile information and configure your authentication.`}>
            </PageTitle>
            <BreadcrumbSetter items={[
                { name: "Settings", url: "/settings/profile" },
                { name: "Profile" },
            ]} />
            <ProfilePasswordChange />
            <ToTpSettings totpEnabled={me.totp_enabled ?? false} />
        </div>
    )
}
