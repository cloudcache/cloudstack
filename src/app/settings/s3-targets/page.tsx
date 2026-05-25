'use server'

import { getAdminUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import { backend } from "@/server/adapter/backend-api.adapter";
import S3TargetsTable from "./s3-targets-table";
import S3TargetEditOverlay from "./s3-target-edit-overlay";
import { Button } from "@/components/ui/button";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";

export default async function S3TargetsPage() {

    await getAdminUserSession();
    const token = await getBackendToken();
    const data = (await backend.adminS3Targets.list(token)) as any[];
    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={'S3 Targets'}
                subtitle={`View all S3 Targets which are configured in the QuickStack Cluster.`}>

                <S3TargetEditOverlay>
                    <Button>Add S3 Target</Button>
                </S3TargetEditOverlay>
            </PageTitle>
            <BreadcrumbSetter items={[
                { name: "Settings", url: "/settings/profile" },
                { name: "S3 Targets" },
            ]} />
            <S3TargetsTable targets={data} />
        </div>
    )
}
