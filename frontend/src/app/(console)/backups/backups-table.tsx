'use client'

import { Button } from "@/components/ui/button";
import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { List } from "lucide-react";
import { BackupInfoModel } from "@/shared/model/backup-info.model";
import { BackupDetailDialog } from "./backup-detail-overlay";
import BackupStatusBadge from "./backup-status-badge";
import { useT } from "@/i18n";

export default function BackupsTable({ data }: { data: BackupInfoModel[] }) {
    const t = useT();

    return <>
        <SimpleDataTable columns={[
            ['projectId', t('common.projectId'), false],
            ['missedBackup', t('common.status'), true, (item) => <BackupStatusBadge missedBackup={item.missedBackup} />],
            ['projectName', t('common.project'), true],
            ['appName', t('common.app'), true],
            ['appId', t('common.appId'), false],
            ['backupVolumeId', t('page.backups.backupVolumeId'), false],
            ['volumeId', t('page.backups.volumeId'), false],
            ['mountPath', t('app.fileMount.mountPath'), true],
            ['backupRetention', t('app.backups.retention'), false],
            ['backupsCount', t('page.backups.title'), true, (item) => t('page.backups.count', { count: item.backups.length })],
            ['item.backups[0].backupDate', t('page.backups.lastBackup'), true, (item) => formatDateTime(item.backups[0].backupDate)],
        ]}
            data={data}
            actionCol={(item) =>
                <>
                    <div className="flex">
                        <div className="flex-1"></div>
                        <BackupDetailDialog backupInfo={item}>
                            <Button variant="ghost" className="h-8 w-8 p-0">
                                <span className="sr-only">{t('page.backups.showBackups')}</span>
                                <List className="h-4 w-4" />
                            </Button>
                        </BackupDetailDialog>
                    </div>
                </>}
        />
    </>
}
