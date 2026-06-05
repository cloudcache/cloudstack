import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Toast } from "@/frontend/utils/toast.utils";
import { Actions } from "@/frontend/utils/nextjs-actions.utils";
import { deleteDbToolDeploymentForAppIfExists, deployDbTool, downloadDbGateFilesForApp, getIsDbToolActive, getLoginCredentialsForRunningDbTool } from "./actions";
import { Label } from "@/components/ui/label";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { Switch } from "@/components/ui/switch";
import { Code } from "@/components/custom/code";
import LoadingSpinner from "@/components/ui/loading-spinner";
import { Download } from "lucide-react";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/i18n";

export default function DbGateDbTool({
    app
}: {
    app: AppExtendedModel;
}) {

    const t = useT();
    const { openConfirmDialog } = useConfirmDialog();
    const [isDbGateActive, setIsDbGateActive] = useState<boolean | undefined>(undefined);
    const [loading, setLoading] = useState(false);

    const loadIsDbGateActive = async (appId: string) => {
        const response = await Actions.run(() => getIsDbToolActive(appId, 'dbgate'));
        setIsDbGateActive(response);
    }

    const downloadDbGateFilesForAppAsync = async () => {
        try {
            setLoading(true);
            await Toast.fromAction(() => downloadDbGateFilesForApp(app.id)).then(x => {
                if (x.status === 'success' && x.data) {
                    window.open('/api/volume-data-download?fileName=' + x.data);
                }
            });
        } finally {
            setLoading(false);
        }
    }

    const openDbGateAsync = async () => {
        try {
            setLoading(true);
            const credentials = await Actions.run(() => getLoginCredentialsForRunningDbTool(app.id, 'dbgate'));
            setLoading(false);
            await openConfirmDialog({
                title: t('app.dbTools.openTitle', { tool: 'DB Gate' }),
                description: <>
                    {t('app.dbTools.readyDescription', { tool: 'DB Gate' })} <br />
                    {t('app.dbTools.credentialsHint')}
                    <div className="pt-3 grid grid-cols-1 gap-1">
                        <Label>{t('common.username')}</Label>
                        <div> <Code>{credentials.username}</Code></div>
                    </div>
                    <div className="pt-3 pb-4 grid grid-cols-1 gap-1">
                        <Label>{t('common.password')}</Label>
                        <div><Code>{credentials.password}</Code></div>
                    </div>
                    <div>
                        <Button variant='outline' onClick={() => window.open(credentials.url, '_blank')}>{t('app.dbTools.openButton', { tool: 'DB Gate' })}</Button>
                    </div>
                </>,
                okButton: '',
                cancelButton: t('common.close')
            });
        } finally {
            setLoading(false);
        }
    }

    useEffect(() => {
        loadIsDbGateActive(app.id);
        return () => {
            setIsDbGateActive(undefined);
        }
    }, [app]);

    return <>
        <div className="flex gap-4 items-center">
            <div className="flex items-center space-x-3">
                <Switch id="canary-channel-mode" disabled={loading || isDbGateActive === undefined} checked={isDbGateActive} onCheckedChange={async (checked) => {
                    try {
                        setLoading(true);
                        if (checked) {
                            await Toast.fromAction(() => deployDbTool(app.id, 'dbgate'), t('app.dbTools.activated', { tool: 'DB Gate' }), t('app.dbTools.activating', { tool: 'DB Gate' }));
                        } else {
                            await Toast.fromAction(() => deleteDbToolDeploymentForAppIfExists(app.id, 'dbgate'), t('app.dbTools.deactivated', { tool: 'DB Gate' }), t('app.dbTools.deactivating', { tool: 'DB Gate' }));
                        }
                        await loadIsDbGateActive(app.id);
                    } finally {
                        setLoading(false);
                    }
                }} />
                <Label htmlFor="airplane-mode">DB Gate</Label>
            </div>
            {isDbGateActive && <>
                <Button variant='outline' onClick={() => openDbGateAsync()}
                    disabled={loading}>{t('app.dbTools.openButton', { tool: 'DB Gate' })}</Button>

                <TooltipProvider>
                    <Tooltip delayDuration={300}>
                        <TooltipTrigger>
                            <Button onClick={() => downloadDbGateFilesForAppAsync()} disabled={!isDbGateActive || loading}
                                variant="ghost"><Download /></Button>
                        </TooltipTrigger>
                        <TooltipContent>
                            <p>{t('app.dbTools.downloadFilesFolder')}</p>
                        </TooltipContent>
                    </Tooltip>
                </TooltipProvider>
            </>}
            {(loading || isDbGateActive === undefined) && <LoadingSpinner></LoadingSpinner>}
        </div>
    </>;
}
