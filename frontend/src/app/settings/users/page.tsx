'use server'

import { getAdminUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import S3TargetEditOverlay from "./user-edit-overlay";
import { Button } from "@/components/ui/button";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import UsersTable from "./users-table";
import { backend } from "@/server/adapter/backend-api.adapter";
import { getT } from "@/i18n/server";

export default async function UsersPage() {

    const session = await getAdminUserSession();
    const token = await getBackendToken();
    const { t } = await getT();
    let users: any[] = [];
    try {
        const result = await backend.adminUsers.list(token);
        users = (result.data ?? []) as any[];
    } catch (e) {
        console.error('[Users] failed to load users:', e);
    }

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={t("settings.users.title")} >
            </PageTitle>
            <BreadcrumbSetter items={[
                { name: t("nav.settings"), url: "/settings/profile" },
                { name: t("settings.users.title") },
            ]} />
            <UsersTable session={session} users={users} userGroups={[]} />
        </div>
    )
}
