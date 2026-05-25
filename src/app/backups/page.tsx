'use server'

import { isAuthorizedForBackups, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import { backend } from "@/server/adapter/backend-api.adapter";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import BackupSchedulesTable from "./backup-schedules-table";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";

export default async function BackupsPage() {
    await isAuthorizedForBackups();
    const token = await getBackendToken();

    let schedules: Awaited<ReturnType<typeof backend.backups.list>> = [];
    let error: string | null = null;

    try {
        schedules = await backend.backups.list(token);
    } catch (e) {
        error = 'Failed to load backup schedules.';
    }

    const activeCount = schedules.filter(s => s.is_active).length;

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={'Backups'}
                subtitle={'View all backup schedules configured for apps and volumes.'}>
            </PageTitle>
            <BreadcrumbSetter items={[{ name: "Backups" }]} />

            <div className="space-y-4">
                {error && (
                    <Alert variant="destructive">
                        <AlertCircle className="h-4 w-4" />
                        <AlertTitle>Error</AlertTitle>
                        <AlertDescription>{error}</AlertDescription>
                    </Alert>
                )}

                {schedules.length === 0 && !error && (
                    <Alert>
                        <AlertCircle className="h-4 w-4" />
                        <AlertTitle>No Backup Schedules</AlertTitle>
                        <AlertDescription>
                            No backup schedules are configured yet. Navigate to an app&apos;s Storage or Credentials tab to set up a backup schedule.
                        </AlertDescription>
                    </Alert>
                )}

                {schedules.length > 0 && (
                    <p className="text-sm text-muted-foreground">
                        {schedules.length} schedule(s) total, {activeCount} active.
                        <span className="ml-2 text-xs">Note: S3 file listing is not yet implemented. Schedules shown are from database.</span>
                    </p>
                )}

                <BackupSchedulesTable schedules={schedules} />
            </div>
        </div>
    );
}
