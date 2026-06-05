'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Download, EditIcon, Folder, TrashIcon, Share2, Unlink2, Unlink } from "lucide-react";
import DialogEditDialog from "./storage-edit-overlay";
import SharedStorageEditDialog from "./shared-storage-edit-overlay";
import { Toast } from "@/frontend/utils/toast.utils";
import { deleteVolume, downloadPvcData, getPvcUsage, openFileBrowserForVolume } from "./actions";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { AppVolume } from "@/shared/model/prisma-compat";
import React from "react";
import { KubeObjectNameUtils } from "@/server/utils/kube-object-name.utils";
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from "@/components/ui/tooltip"
import { Code } from "@/components/custom/code";
import { Label } from "@/components/ui/label";
import { KubeSizeConverter } from "@/shared/utils/kubernetes-size-converter.utils";
import { Progress } from "@/components/ui/progress";
import { NodeInfoModel } from "@/shared/model/node-info.model";
import { toast } from "sonner";
import { useT } from "@/i18n";

type AppVolumeWithCapacity = (AppVolume & {
    usedBytes?: number;
    capacityBytes?: number;
    usedPercentage?: number;
});

export default function StorageList({ app, readonly, nodesInfo }: {
    app: AppExtendedModel;
    nodesInfo: NodeInfoModel[];
    readonly: boolean;
}) {
    const t = useT();

    const appVolumes = (app.appVolumes ?? []) as AppVolumeWithCapacity[];
    const [volumesWithStorage, setVolumesWithStorage] = React.useState<AppVolumeWithCapacity[]>(appVolumes);
    const [isLoading, setIsLoading] = React.useState(false);

    const loadAndMapStorageData = async () => {

        const response = (await getPvcUsage(app.id, app.projectId));

        if (response.status === 'success' && response.data) {
            const usageItems = Array.isArray(response.data) ? response.data : [];
            const mappedVolumeData = [...appVolumes];
            for (let item of mappedVolumeData) {
                const volume = usageItems.find((x: any) => x.pvcName === KubeObjectNameUtils.toPvcName(item.sharedVolumeId || item.id));
                if (volume) {
                    item.usedBytes = volume.usedBytes;
                    item.capacityBytes = KubeSizeConverter.fromMegabytesToBytes(item.size);
                    item.usedPercentage = Math.round(volume.usedBytes / item.capacityBytes * 100);
                }
            }
            setVolumesWithStorage(mappedVolumeData);
        } else {
            toast.error(response.message ?? t('app.storage.usageLoadFailed'));
        }
    }

    React.useEffect(() => {
        loadAndMapStorageData();
    }, [app]);

    const { openConfirmDialog: openDialog } = useConfirmDialog();

    const asyncDeleteVolume = async (volumeId: string, isBaseVolume: boolean) => {
        try {
            const confirm = await openDialog({
                title: isBaseVolume ? t('app.storage.deleteVolume') : t('app.storage.detachVolume'),
                description: isBaseVolume ? t('app.storage.deleteDescription') : t('app.storage.detachDescription'),
                okButton: isBaseVolume ? t('app.storage.deleteVolume') : t('app.storage.detachVolume')
            });
            if (confirm) {
                setIsLoading(true);
                await Toast.fromAction(() => deleteVolume(volumeId));
            }
        } finally {
            setIsLoading(false);
        }
    };

    const asyncDownloadPvcData = async (volumeId: string) => {
        try {
            const confirm = await openDialog({
                title: t('app.storage.downloadTitle'),
                description: t('app.storage.downloadDescription'),
                okButton: t('common.download')
            });
            if (confirm) {
                setIsLoading(true);
                await Toast.fromAction(() => downloadPvcData(volumeId)).then(x => {
                    if (x.status === 'success' && x.data) {
                        window.open('/api/volume-data-download?fileName=' + x.data);
                    }
                });
            }
        } finally {
            setIsLoading(false);
        }
    }

    const openFileBrowserForVolumeAsync = async (volumeId: string) => {

        try {
            const confirm = await openDialog({
                title: t('app.storage.openFileBrowser'),
                description: t('app.storage.openFileBrowserDescription'),
                okButton: t('app.storage.stopAndOpenFileBrowser')
            });
            if (!confirm) {
                return;
            }
            setIsLoading(true);
            const fileBrowserStartResult = await Toast.fromAction(() => openFileBrowserForVolume(volumeId), undefined, t('app.storage.startingFileBrowser'))
            if (fileBrowserStartResult.status !== 'success' || !fileBrowserStartResult.data) {
                return;
            }
            await openDialog({
                title: t('app.storage.fileBrowserReady'),
                description: <>
                    {t('app.storage.fileBrowserReadyDescription')} <br />
                    {t('app.storage.fileBrowserCredentials')}
                    <div className="pt-3 grid grid-cols-1 gap-1">
                        <Label>{t('common.username')}</Label>
                        <div> <Code>quickstack</Code></div>
                    </div>
                    <div className="pt-3 pb-4 grid grid-cols-1 gap-1">
                        <Label>{t('auth.password')}</Label>
                        <div><Code>{fileBrowserStartResult.data.password}</Code></div>
                    </div>
                    <div>
                        <Button variant='outline' onClick={() => window.open(fileBrowserStartResult.data!.url, '_blank')}>{t('app.storage.openFileBrowser')}</Button>
                    </div>
                </>,
                okButton: '',
                cancelButton: t('common.close')
            });
        } finally {
            setIsLoading(false);
        }
    }

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.storage.volumes')}</CardTitle>
                <CardDescription>{t('app.storage.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('app.storage.count', { count: appVolumes.length })}</TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('app.storage.mountPath')}</TableHead>
                            <TableHead>{t('app.storage.storageSize')}</TableHead>
                            <TableHead>{t('app.storage.storageUsed')}</TableHead>
                            <TableHead>{t('app.storage.storageClass')}</TableHead>
                            <TableHead>{t('app.storage.accessMode')}</TableHead>
                            <TableHead>{t('app.storage.shared')}</TableHead>
                            <TableHead className="w-[100px]">{t('common.actions')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {volumesWithStorage.map(volume => (
                            <TableRow key={volume.containerMountPath}>
                                <TableCell className="font-medium">{volume.containerMountPath}</TableCell>
                                <TableCell className="font-medium">{volume.size} MB</TableCell>
                                <TableCell className="font-medium space-y-2">
                                    {volume.usedPercentage && <>
                                        <Progress value={volume.usedPercentage}
                                            color={volume.usedPercentage >= 90 ? 'red' : (volume.usedPercentage >= 80 ? 'orange' : undefined)} />
                                        <div className='text-xs text-slate-500'>
                                            {t('app.storage.used', { value: KubeSizeConverter.convertBytesToReadableSize(volume.usedBytes!), percent: volume.usedPercentage })}
                                        </div>
                                    </>}
                                </TableCell>
                                <TableCell className="font-medium capitalize">{volume.storageClassName?.replace('-', ' ')}</TableCell>
                                <TableCell className="font-medium">{volume.accessMode}</TableCell>
                                <TableCell className="font-medium">
                                    {volume.shareWithOtherApps && (
                                        <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <span className="px-2 py-1 rounded-lg text-xs font-semibold bg-green-100 text-green-800 inline-flex items-center gap-1">
                                                        <Share2 className="h-3 w-3" />
                                                        {t('app.storage.shareable')}
                                                    </span>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{t('app.storage.shareableTooltip')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                    )}
                                    {volume.sharedVolumeId && (
                                        <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <span className="px-2 py-1 rounded-lg text-xs font-semibold bg-blue-100 text-blue-800 inline-flex items-center gap-1">
                                                        <Share2 className="h-3 w-3" />
                                                        {t('app.storage.shared')}
                                                    </span>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{t('app.storage.sharedTooltip')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                    )}
                                </TableCell>
                                <TableCell className="font-medium flex gap-2">
                                    {!volume.sharedVolumeId && <>
                                        <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <Button variant="ghost" onClick={() => asyncDownloadPvcData(volume.id)} disabled={isLoading}>
                                                        <Download />
                                                    </Button>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{t('app.storage.downloadContent')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                        {!readonly && <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <Button variant="ghost" onClick={() => openFileBrowserForVolumeAsync(volume.id)} disabled={isLoading}>
                                                        <Folder />
                                                    </Button>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{t('app.storage.viewContent')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>}
                                    </>}
                                    {/*<StorageRestoreDialog app={app} volume={volume}>
                                        <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <Button variant="ghost" disabled={isLoading}>
                                                        <Upload />
                                                    </Button>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{t('app.storageRestore.restoreFromZip')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                    </StorageRestoreDialog>*/}
                                    {!readonly && <>
                                        {volume.sharedVolumeId ? (
                                            <TooltipProvider>
                                                <Tooltip delayDuration={200}>
                                                    <TooltipTrigger>
                                                        <Button variant="ghost" disabled={true}><EditIcon /></Button>
                                                    </TooltipTrigger>
                                                    <TooltipContent>
                                                        <p>{t('app.storage.sharedCannotEdit')}</p>
                                                    </TooltipContent>
                                                </Tooltip>
                                            </TooltipProvider>
                                        ) : (
                                            <DialogEditDialog app={app} volume={volume} nodesInfo={nodesInfo}>
                                                <TooltipProvider>
                                                    <Tooltip delayDuration={200}>
                                                        <TooltipTrigger>
                                                            <Button variant="ghost" disabled={isLoading}><EditIcon /></Button>
                                                        </TooltipTrigger>
                                                        <TooltipContent>
                                                            <p>{t('app.storage.editSettings')}</p>
                                                        </TooltipContent>
                                                    </Tooltip>
                                                </TooltipProvider>
                                            </DialogEditDialog>
                                        )}
                                        <TooltipProvider>
                                            <Tooltip delayDuration={200}>
                                                <TooltipTrigger>
                                                    <Button variant="ghost" onClick={() => asyncDeleteVolume(volume.id, !volume.sharedVolumeId)} disabled={isLoading}>
                                                        {volume.sharedVolumeId ? <Unlink /> : <TrashIcon />}
                                                    </Button>
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p>{volume.sharedVolumeId ? t('app.storage.detachVolume') : t('app.storage.deleteVolume')}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                    </>}
                                </TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
            {!readonly && <CardFooter className="flex gap-2">
                <DialogEditDialog app={app} nodesInfo={nodesInfo}>
                    <Button>{t('app.storage.addVolume')}</Button>
                </DialogEditDialog>
                <SharedStorageEditDialog app={app}>
                    <Button variant="outline">{t('app.storage.addSharedVolume')}</Button>
                </SharedStorageEditDialog>
            </CardFooter>}
        </Card >
    </>;
}
