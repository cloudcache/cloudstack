import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useEffect, useState } from "react";
import LogsStreamed from "@/components/custom/logs-streamed";
import { getPodsForApp as getPodsForAppAction } from "./actions";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { toast } from "sonner";
import { LogsDialog } from "@/components/custom/logs-overlay";
import { Button } from "@/components/ui/button";
import { Download, Expand, Terminal } from "lucide-react";
import { TerminalDialog } from "./terminal-overlay";
import { LogsDownloadOverlay } from "./logs-download-overlay";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { usePodsStatus } from "@/frontend/states/zustand.states";
import { backend } from "@/server/adapter/backend-api.adapter";
import { useT } from "@/i18n";

const errorMessage = (value: unknown, fallback: string) => {
    if (value instanceof Error) return value.message;
    if (typeof value === 'object' && value && 'message' in value && typeof value.message === 'string') return value.message;
    return fallback;
};

export default function Logs({
    app,
    role
}: {
    app: any;
    role: RolePermissionEnum;
}) {
    const t = useT();
    const [selectedPodName, setSelectedPodName] = useState<string | undefined>(undefined);
    const [appPods, setAppPods] = useState<{ name: string; phase?: string }[] | undefined>(undefined);
    const { subscribeToStatusChanges } = usePodsStatus();

    const projectId: string = app.project_id ?? app.projectId ?? '';
    const appId: string = app.id ?? '';

    const updatePods = async () => {
        if (!projectId || !appId) return;
        try {
            const response = await getPodsForAppAction(projectId, appId);
            if (response.status === 'success' && response.data) {
                const pods = (response.data as any[]).map((p: any) => ({
                    name: p.name ?? p.podName,
                    phase: p.phase ?? p.status,
                }));
                setAppPods(pods);
                if (pods.length > 0 && !selectedPodName) {
                    setSelectedPodName(pods[0].name);
                }
            } else {
                setAppPods([]);
                toast.error(errorMessage(response, t('app.logs.podsLoadFailed')));
            }
        } catch (ex) {
            setAppPods([]);
            toast.error(errorMessage(ex, t('app.logs.podsLoadFailed')));
        }
    }

    useEffect(() => {
        updatePods();
        const unsubscribe = subscribeToStatusChanges((changedAppIds) => {
            if (changedAppIds.includes(appId)) {
                setTimeout(() => updatePods(), 500);
                setTimeout(() => updatePods(), 10000);
            }
        });
        return () => unsubscribe();
    }, [appId]);

    // Build Rust SSE URL for the entire app's logs (not per-pod — Rust aggregates)
    const logsUrl = projectId && appId
        ? backend.apps.logsUrl(projectId, appId)
        : undefined;

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.logs.title')}</CardTitle>
                <CardDescription>{t('app.logs.description')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                {!appPods && <FullLoadingSpinner />}
                {appPods && appPods.length === 0 && <div>{t('app.logs.noPods')}</div>}
                {appPods && appPods.length > 0 && <div className="flex gap-4">
                    <div className="flex-1">
                        <Select value={selectedPodName} onValueChange={setSelectedPodName}>
                            <SelectTrigger className="w-full">
                                <SelectValue placeholder={t('app.logs.selectPod')} />
                            </SelectTrigger>
                            <SelectContent>
                                {appPods.map(pod => (
                                    <SelectItem key={pod.name} value={pod.name}>
                                                {pod.name} ({pod.phase ?? t('common.unknown')})
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>
                    {role === RolePermissionEnum.READWRITE && <div>
                        <TerminalDialog terminalInfo={{
                            podName: selectedPodName ?? '',
                            containerName: '',
                            namespace: projectId,
                        }}>
                            <Button variant="secondary">
                                <Terminal /> Terminal
                            </Button>
                        </TerminalDialog>
                    </div>}
                    <div>
                        <TooltipProvider>
                            <Tooltip delayDuration={300}>
                                <TooltipTrigger>
                                    <LogsDownloadOverlay appId={appId}>
                                        <Button variant="secondary">
                                            <Download />
                                        </Button>
                                    </LogsDownloadOverlay>
                                </TooltipTrigger>
                                <TooltipContent><p>{t('app.logs.downloadLogs')}</p></TooltipContent>
                            </Tooltip>
                        </TooltipProvider>
                    </div>
                    <div>
                        <Tooltip delayDuration={300}>
                            <TooltipTrigger>
                                <LogsDialog logsUrl={logsUrl}>
                                    <Button variant="secondary"><Expand /></Button>
                                </LogsDialog>
                            </TooltipTrigger>
                            <TooltipContent><p>{t('app.logs.fullscreenLogs')}</p></TooltipContent>
                        </Tooltip>
                    </div>
                </div>}
                {logsUrl && <LogsStreamed logsUrl={logsUrl} />}
            </CardContent>
        </Card>
    </>;
}
