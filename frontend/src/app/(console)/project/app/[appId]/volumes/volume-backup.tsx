'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { EditIcon, Play, TrashIcon } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { deleteBackupVolume, runBackupVolumeSchedule } from "./actions";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { S3Target } from "@/shared/model/prisma-compat";
import React from "react";
import { formatDateTime } from "@/frontend/utils/format.utils";
import VolumeBackupEditDialog from "./volume-backup-edit-overlay";
import { VolumeBackupExtendedModel } from "@/shared/model/volume-backup-extended.model";
import { AppVolume } from "@/shared/model/prisma-compat";
import { useT } from "@/i18n";

export default function VolumeBackupList({
    app,
    volumeBackups,
    s3Targets,
    readonly
}: {
    app: AppExtendedModel,
    s3Targets: S3Target[],
    volumeBackups: VolumeBackupExtendedModel[];
    readonly: boolean;
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const [isLoading, setIsLoading] = React.useState(false);

    // Filter out shared volumes (volumes that are mounted from other apps)
    const ownVolumes = app.appVolumes.filter(volume => !volume.sharedVolumeId) as AppVolume[];

    const asyncDeleteBackupVolume = async (volumeId: string) => {
        const confirm = await openDialog({
            title: t('app.backups.deleteTitle'),
            description: t('app.backups.deleteDescription'),
            okButton: t('app.backups.deleteButton')
        });
        if (confirm) {
            await Toast.fromAction(() => deleteBackupVolume(volumeId));
        }
    };

    const asyncRunBackupVolumeSchedule = async (volumeId: string) => {
        const confirm = await openDialog({
            title: t('app.backups.createTitle'),
            description: t('app.backups.createDescription'),
            okButton: t('app.backups.createButton')
        });
        setIsLoading(true);
        try {
            if (confirm) {
                await Toast.fromAction(() => runBackupVolumeSchedule(volumeId), undefined, t('app.backups.creating'));
            }
        } finally {
            setIsLoading(false);
        }
    };

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.backups.title')}</CardTitle>
                <CardDescription>{t('app.backups.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('app.backups.count', { count: volumeBackups.length })}</TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('app.backups.cronExpression')}</TableHead>
                            <TableHead>{t('app.backups.retention')}</TableHead>
                            <TableHead>{t('app.backups.method')}</TableHead>
                            <TableHead>{t('app.backups.backupLocation')}</TableHead>
                            <TableHead>{t('common.createdAt')}</TableHead>
                            <TableHead className="w-[100px]">{t('common.action')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {volumeBackups.map(volumeBackup => (
                            <TableRow key={volumeBackup.id}>
                                <TableCell className="font-medium">{volumeBackup.cron}</TableCell>
                                <TableCell className="font-medium">{volumeBackup.retention}</TableCell>
                                <TableCell className="font-medium">
                                    {app.appType !== 'APP' && volumeBackup.useDatabaseBackup
                                        ? t('app.backups.databaseMethod', { type: app.appType.toLocaleLowerCase() })
                                        : t('app.backups.volumeArchiveMethod')}
                                </TableCell>
                                <TableCell className="font-medium">{volumeBackup.target.name}</TableCell>
                                <TableCell className="font-medium">{formatDateTime(volumeBackup.createdAt)}</TableCell>
                                {!readonly && <TableCell className="font-medium flex gap-2">
                                    <Button disabled={isLoading} variant="ghost" onClick={() => asyncRunBackupVolumeSchedule(volumeBackup.id)}>
                                        <Play />
                                    </Button>
                                    <VolumeBackupEditDialog volumeBackup={volumeBackup}
                                        s3Targets={s3Targets} volumes={ownVolumes as AppVolume[]} app={app}>
                                        <Button disabled={isLoading} variant="ghost"><EditIcon /></Button>
                                    </VolumeBackupEditDialog>
                                    <Button disabled={isLoading} variant="ghost" onClick={() => asyncDeleteBackupVolume(volumeBackup.id)}>
                                        <TrashIcon />
                                    </Button>
                                </TableCell>}
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
            {!readonly && <CardFooter>
                <VolumeBackupEditDialog s3Targets={s3Targets} volumes={ownVolumes as AppVolume[]} app={app}>
                    <Button>{t('app.backups.add')}</Button>
                </VolumeBackupEditDialog>
            </CardFooter>}
        </Card >
    </>;
}
