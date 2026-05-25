'use server'

import { getAdminUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import S3TargetEditOverlay from "./user-edit-overlay";
import { Button } from "@/components/ui/button";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import UsersTable from "./users-table";
import { backend } from "@/server/adapter/backend-api.adapter";

export default async function UsersPage() {

    const session = await getAdminUserSession();
    const token = await getBackendToken();
    const result = await backend.adminUsers.list(token);
    const users = (result.data ?? []) as any[];

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={'Users'} >
            </PageTitle>
            <BreadcrumbSetter items={[
                { name: "Settings", url: "/settings/profile" },
                { name: "Users" },
            ]} />
            <UsersTable session={session} users={users} userGroups={[]} />
        </div>
    )
}
