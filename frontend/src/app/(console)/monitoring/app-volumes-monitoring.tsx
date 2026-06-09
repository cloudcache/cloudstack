'use client';


import {
    Card,
    CardContent,
    CardHeader,
    CardTitle,
} from '@/components/ui/card';
import { useEffect, useState } from 'react';
import { Actions } from '@/frontend/utils/nextjs-actions.utils';
import { getVolumeMonitoringUsage } from './actions';
import { toast } from 'sonner';
import FullLoadingSpinner from '@/components/ui/full-loading-spinnter';
import { AppVolumeMonitoringUsageModel } from '@/shared/model/app-volume-monitoring-usage.model';
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { KubeSizeConverter } from '@/shared/utils/kubernetes-size-converter.utils';
import { ExternalLink } from 'lucide-react';
import Link from 'next/link';
import { Button } from '@/components/ui/button';
import { Progress } from "@/components/ui/progress"
import { useT } from '@/i18n';

type AppVolumeMonitoringUsageExtendedModel = AppVolumeMonitoringUsageModel & {
    usedPercentage: number;
};

export default function AppVolumeMonitoring({
    volumesUsage
}: {
    volumesUsage?: AppVolumeMonitoringUsageModel[]
}) {
    const t = useT();

    const convertToExtendedModel = (input?: AppVolumeMonitoringUsageModel[]): AppVolumeMonitoringUsageExtendedModel[] | undefined => {
        if (input) {
            return input.map(item => ({
                ...item,
                usedPercentage: Math.round(item.usedBytes / item.capacityBytes * 100)
            }));
        }
        return undefined;
    }

    const [totalUsedBytes, setTotalUsedBytes] = useState<number | undefined>(undefined);
    const [totalCapacityBytes, setTotalCapacityBytes] = useState<number | undefined>(undefined);

    const [updatedVolumeUsage, setUpdatedVolumeUsage] = useState<AppVolumeMonitoringUsageExtendedModel[] | undefined>(convertToExtendedModel(volumesUsage));

    const fetchVolumeMonitoringUsage = async () => {
        try {
            let data = await Actions.run(() => getVolumeMonitoringUsage());
            data  = data?.filter((volume) => !!volume.isBaseVolume);
            setUpdatedVolumeUsage(convertToExtendedModel(data));
            setUsedAndCapacityBytes(convertToExtendedModel(data));
        } catch (ex) {
            toast.error(t('monitoring.fetchCurrentUsageFailed'));
            console.error('An error occurred while fetching volume nodes', ex);
        }
    }

    const setUsedAndCapacityBytes = (input?: AppVolumeMonitoringUsageExtendedModel[]) => {
        if (input) {
            const totalUsed = input.reduce((acc, item) => acc + item.usedBytes, 0);
            const totalCapacity = input.reduce((acc, item) => acc + item.capacityBytes, 0);
            setTotalUsedBytes(totalUsed);
            setTotalCapacityBytes(totalCapacity);
        }
    }

    useEffect(() => {
        const volumeUsageId = setInterval(() => fetchVolumeMonitoringUsage(), 10000);
        setUsedAndCapacityBytes(convertToExtendedModel(volumesUsage));
        return () => {
            clearInterval(volumeUsageId);
        }
    }, [volumesUsage]);

    if (!updatedVolumeUsage) {
        return <Card>
            <CardHeader>
                <CardTitle>{t('monitoring.appVolumesCapacity')}</CardTitle>
            </CardHeader>
            <CardContent>
                <FullLoadingSpinner />
            </CardContent>
        </Card>
    }

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('monitoring.appVolumesCapacity')}</CardTitle>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('monitoring.appVolumesCount', { count: updatedVolumeUsage.length })} {totalUsedBytes && totalCapacityBytes && <>
                        <span className='text-slate-500'> | {t('monitoring.totalUsed')} {KubeSizeConverter.convertBytesToReadableSize(totalUsedBytes)} | {t('monitoring.totalAllocated')} {KubeSizeConverter.convertBytesToReadableSize(totalCapacityBytes)}</span>
                    </>}
                    </TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('common.project')}</TableHead>
                            <TableHead>{t('common.app')}</TableHead>
                            <TableHead>{t('app.fileMount.mountPath')}</TableHead>
                            <TableHead>{t('monitoring.capacity')}</TableHead>
                            <TableHead></TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {updatedVolumeUsage.map((item, index) => (
                            <TableRow key={item.appId}>
                                <TableCell>{item.projectName}</TableCell>
                                <TableCell>{item.appName}</TableCell>
                                <TableCell>{item.mountPath}</TableCell>
                                <TableCell className='space-y-1'>
                                    <Progress value={item.usedPercentage}
                                        color={item.usedPercentage >= 90 ? 'red' : (item.usedPercentage >= 80 ? 'orange' : undefined)} />
                                    <div className='text-xs text-slate-500'>{KubeSizeConverter.convertBytesToReadableSize(item.usedBytes)} / {KubeSizeConverter.convertBytesToReadableSize(item.capacityBytes)}</div>
                                </TableCell>
                                <TableCell>
                                    <Link href={`/project/app/${item.appId}?tabName=storage`} >
                                        <Button variant="ghost" size="sm">
                                            <ExternalLink />
                                        </Button>
                                    </Link>
                                </TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
        </Card>
    );
}
