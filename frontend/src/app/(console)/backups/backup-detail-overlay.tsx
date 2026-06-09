import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog"
import React from "react";
import { BackupInfoModel } from "@/shared/model/backup-info.model";
import { ScrollArea } from "@radix-ui/react-scroll-area";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { KubeSizeConverter } from "@/shared/utils/kubernetes-size-converter.utils";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { deleteBackup, downloadBackup } from "./actions";
import { Button } from "@/components/ui/button";
import { Download, Trash2 } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { useT } from "@/i18n";

export function BackupDetailDialog({
    backupInfo,
    children
}: {
    backupInfo: BackupInfoModel;
    children: React.ReactNode;
}) {

    const { openConfirmDialog } = useConfirmDialog();
    const t = useT();
    const [isOpen, setIsOpen] = React.useState(false);
    const [isLoading, setIsLoading] = React.useState(false);

    const asyncDownloadPvcData = async (s3Key: string) => {
        try {
            setIsLoading(true);
            await Toast.fromAction(() => downloadBackup(backupInfo.s3TargetId, s3Key)).then(x => {
                if (x.status === 'success' && x.data) {
                    window.open('/api/volume-data-download?fileName=' + x.data);
                }
            });
        } finally {
            setIsLoading(false);
        }
    }

    const asyncDeleteBackup = async (s3Key: string) => {
        if (await openConfirmDialog({
            title: t('page.backups.deleteTitle'),
            description: t('page.backups.deleteDescription'),
            okButton: t('common.delete')
        })) {
            await Toast.fromAction(() => deleteBackup(backupInfo.s3TargetId, s3Key));
        }
    }

    return (
        <Dialog open={isOpen} onOpenChange={(isO) => {
            setIsOpen(isO);
        }}>
            <DialogTrigger asChild>
                {children}
            </DialogTrigger>
            <DialogContent className="sm:max-w-[700px]">
                <DialogHeader>
                    <DialogTitle>{t("page.backups.title")}</DialogTitle>
                    <DialogDescription>
                        <span className="font-semibold">{t("page.backups.app")}</span> {backupInfo.appName}<br />
                        <span className="font-semibold">{t("page.backups.mountPath")}</span> {backupInfo.mountPath}<br />
                        {t('page.backups.retentionDescription', { count: backupInfo.backupRetention })}
                    </DialogDescription>
                </DialogHeader>
                <ScrollArea className="max-h-[70vh]">
                    <Table>
                        <TableCaption>{t('page.backups.count', { count: backupInfo.backups.length })}</TableCaption>
                        <TableHeader>
                            <TableRow>
                                <TableHead>{t("page.backups.time")}</TableHead>
                                <TableHead>{t("page.backups.size")}</TableHead>
                                <TableHead></TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {backupInfo.backups.map((item, index) => (
                                <TableRow key={index}>
                                    <TableCell>{formatDateTime(item.backupDate, true)}</TableCell>
                                    <TableCell>{item.sizeBytes ? KubeSizeConverter.convertBytesToReadableSize(item.sizeBytes) : t('common.unknown')}</TableCell>
                                    <TableCell className="flex justify-end gap-2">
                                        <Button variant="ghost" size="sm" onClick={() => asyncDownloadPvcData(item.key)} disabled={isLoading}>
                                            <Download />
                                        </Button>
                                        <Button variant="ghost" size="sm" onClick={() => asyncDeleteBackup(item.key)} disabled={isLoading}>
                                            <Trash2 />
                                        </Button>
                                    </TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </ScrollArea>
            </DialogContent>
        </Dialog>
    )
}
