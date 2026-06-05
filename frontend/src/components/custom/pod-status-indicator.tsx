'use client'

import { usePodsStatus } from '@/frontend/states/zustand.states';
import { cn } from '@/frontend/utils/utils';
import { Spinner } from "@/components/ui/spinner"
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip"
import { useT } from "@/i18n";

interface PodStatusIndicatorProps {
    appId: string;
    showLabel?: boolean;
}

export default function PodStatusIndicator({ appId, showLabel }: PodStatusIndicatorProps) {
    const t = useT();
    const { getPodsForApp, isLoading } = usePodsStatus();
    const appPods = getPodsForApp(appId);

    if (isLoading) {
        return (
            <div className="flex items-center gap-2">
                <Spinner className="size-3" />
            </div>
        );
    }

    if (!appPods) {
        return (
            <Tooltip>
                <TooltipTrigger asChild>
                    <div className="flex items-center gap-2 w-fit">
                        <div className="w-3 h-3 rounded-full bg-red-400" />
                        {showLabel && <span className="text-xs text-gray-500">{t('common.unknown')}</span>}
                    </div>
                </TooltipTrigger>
                <TooltipContent>
                    <p>{t('app.status.retrieveFailed')}</p>
                </TooltipContent>
            </Tooltip>
        );
    }

    let statusColor = 'bg-gray-400';
    const runningPods = appPods.readyReplicas ?? 0;
    const expected = appPods.replicas ?? 0;
    let statusLabel = `${runningPods}/${expected}`;
    let tooltipText = t('app.status.podsRunning', { running: runningPods, expected });

    if (appPods.deploymentStatus === 'SHUTDOWN') {
        statusColor = 'bg-gray-400';
        statusLabel = t('app.status.off');
        tooltipText = t('app.status.shutDown');
    }

    if (appPods.deploymentStatus === 'DEPLOYING' || appPods.deploymentStatus === 'SHUTTING_DOWN') {
        statusColor = 'bg-orange-500';
    }

    if (appPods.deploymentStatus === 'DEPLOYED') {
        statusColor = 'bg-green-500';
        statusLabel = t('app.status.ok');
    }

    if (appPods.deploymentStatus === 'ERROR') {
        statusColor = 'bg-red-500';
        statusLabel = t('app.status.error');
        tooltipText = t('app.status.errorDuringDeployment');
    }

    if (appPods.deploymentStatus === 'BUILDING') {
        statusColor = 'bg-blue-500';
        statusLabel = t('app.status.build');
        tooltipText = t('app.status.building');
    }

    if (appPods.deploymentStatus === 'UNKNOWN') {
        statusColor = 'bg-gray-400';
        statusLabel = t('common.unknown');
        tooltipText = t('app.status.unknownDeployment');
    }

    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <div className="flex items-center gap-2 w-fit">
                    <div className={cn("w-3 h-3 rounded-full", statusColor)} />
                    {showLabel && <span className="text-xs text-gray-700">{statusLabel}</span>}
                </div>
            </TooltipTrigger>
            <TooltipContent>
                <p>{tooltipText}</p>
            </TooltipContent>
        </Tooltip>
    );
}
