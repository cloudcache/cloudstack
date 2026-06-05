'use server'

import { isAuthorizedForBackups, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import { backend } from "@/server/adapter/backend-api.adapter";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import BackupSchedulesTable from "./backup-schedules-table";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";
import { getT } from "@/i18n/server";

export default async function BackupsPage() {
    await isAuthorizedForBackups();
    const token = await getBackendToken();
    const { t } = await getT();

    let schedules: Awaited<ReturnType<typeof backend.backups.list>> = [];
    let error: string | null = null;

    try {
        schedules = await backend.backups.list(token);
    } catch (e) {
        error = t('page.backups.loadFailed');
    }

    const activeCount = schedules.filter(s => s.is_active).length;

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={t("page.backups.title")}
                subtitle={t("page.backups.subtitle")}>
            </PageTitle>
            <BreadcrumbSetter items={[{ name: t("page.backups.title") }]} />

            <div className="space-y-4">
                {error && (
                    <Alert variant="destructive">
                        <AlertCircle className="h-4 w-4" />
                        <AlertTitle>{t('common.error')}</AlertTitle>
                        <AlertDescription>{error}</AlertDescription>
                    </Alert>
                )}

                {schedules.length === 0 && !error && (
                    <Alert>
                        <AlertCircle className="h-4 w-4" />
                        <AlertTitle>{t('page.backups.emptyTitle')}</AlertTitle>
                        <AlertDescription>
                            {t('page.backups.emptyDescription')}
                        </AlertDescription>
                    </Alert>
                )}

                {schedules.length > 0 && (
                    <p className="text-sm text-muted-foreground">
                        {t('page.backups.scheduleSummary', { total: schedules.length, active: activeCount })}
                        <span className="ml-2 text-xs">{t("page.backups.note")}</span>
                    </p>
                )}

                <BackupSchedulesTable schedules={schedules} />
            </div>
        </div>
    );
}
