import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Toast } from "@/frontend/utils/toast.utils";
import { Actions } from "@/frontend/utils/nextjs-actions.utils";
import { DbToolIds, deleteDbToolDeploymentForAppIfExists, deployDbTool, getIsDbToolActive, getLoginCredentialsForRunningDbTool } from "./actions";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Code } from "@/components/custom/code";
import LoadingSpinner from "@/components/ui/loading-spinner";
import { useT } from "@/i18n";

export default function DbToolSwitch({
    app,
    toolId,
    toolNameString
}: {
    app: AppExtendedModel;
    toolId: DbToolIds;
    toolNameString: string;
}) {

    const t = useT();
    const { openConfirmDialog } = useConfirmDialog();
    const [isDbToolActive, setIsDbToolActive] = useState<boolean | undefined>(undefined);
    const [loading, setLoading] = useState(false);

    const loadIdDbToolActive = async (appId: string) => {
        const response = await Actions.run(() => getIsDbToolActive(appId, toolId));
        setIsDbToolActive(response);
    }

    const openDbTool = async () => {
        try {
            setLoading(true);
            const credentials = await Actions.run(() => getLoginCredentialsForRunningDbTool(app.id, toolId));
            setLoading(false);
            await openConfirmDialog({
                title: t('app.dbTools.openGenericTitle'),
                description: <>
                    {t('app.dbTools.readyDescription', { tool: toolNameString })} <br />
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
                        <Button variant='outline' onClick={() => window.open(credentials.url, '_blank')}>{t('app.dbTools.openButton', { tool: toolNameString })}</Button>
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
        loadIdDbToolActive(app.id);
        return () => {
            setIsDbToolActive(undefined);
        }
    }, [app]);

    return <>
        <div className="flex gap-4 items-center">
            <div className="flex items-center space-x-3">
                <Switch id="canary-channel-mode" disabled={loading || isDbToolActive === undefined} checked={isDbToolActive} onCheckedChange={async (checked) => {
                    try {
                        setLoading(true);
                        if (checked) {
                            await Toast.fromAction(() => deployDbTool(app.id, toolId), t('app.dbTools.activated', { tool: toolNameString }), t('app.dbTools.activating', { tool: toolNameString }));
                        } else {
                            await Toast.fromAction(() => deleteDbToolDeploymentForAppIfExists(app.id, toolId), t('app.dbTools.deactivated', { tool: toolNameString }), t('app.dbTools.deactivating', { tool: toolNameString }));
                        }
                        await loadIdDbToolActive(app.id);
                    } finally {
                        setLoading(false);
                    }
                }} />
                <Label htmlFor="airplane-mode">{toolNameString}</Label>
            </div>
            {isDbToolActive && <>
                <Button variant='outline' onClick={() => openDbTool()}
                    disabled={!isDbToolActive || loading}>{t('app.dbTools.openButton', { tool: toolNameString })}</Button>
            </>}
            {(loading || isDbToolActive === undefined) && <LoadingSpinner></LoadingSpinner>}
        </div>
    </>;
}
