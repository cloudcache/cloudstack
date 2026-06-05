import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { deleteBuild, getDeploymentsAndBuildsForApp } from "./actions";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { Button } from "@/components/ui/button";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Toast } from "@/frontend/utils/toast.utils";
import { DeploymentInfoModel } from "@/shared/model/deployment-info.model";
import DeploymentStatusBadge from "./deployment-status-badge";
import { BuildLogsDialog } from "./build-logs-overlay";
import ShortCommitHash from "@/components/custom/short-commit-hash";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { useT } from "@/i18n";

const errorMessage = (value: unknown, fallback: string) => {
    if (value instanceof Error) return value.message;
    if (typeof value === 'object' && value && 'message' in value && typeof value.message === 'string') return value.message;
    return fallback;
};

export default function BuildsTab({
    app,
    role
}: {
    app: AppExtendedModel;
    role: RolePermissionEnum;
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const [appBuilds, setAppBuilds] = useState<DeploymentInfoModel[] | undefined>(undefined);
    const [error, setError] = useState<string | undefined>(undefined);
    const [selectedDeploymentForLogs, setSelectedDeploymentForLogs] = useState<DeploymentInfoModel | undefined>(undefined);

    const updateBuilds = async () => {
        setError(undefined);
        try {
            const response = await getDeploymentsAndBuildsForApp(app.id);
            if (response.status === 'success' && response.data) {
                setAppBuilds(response.data);
            } else {
                setAppBuilds([]);
                setError(errorMessage(response, t('app.deployments.loadFailed')));
            }
        } catch (ex) {
            setAppBuilds([]);
            setError(errorMessage(ex, t('app.deployments.loadFailed')));
        }
    }

    const deleteBuildClick = async (buildName: string) => {
        const confirm = await openDialog({
            title: t('app.deployments.deleteBuildTitle'),
            description: t('app.deployments.deleteBuildDescription'),
            okButton: t('app.deployments.stopRemoveBuild')
        });
        if (confirm) {
            await Toast.fromAction(() => deleteBuild(buildName));
            await updateBuilds();
        }
    }

    useEffect(() => {
        if (app.sourceType === 'container') {
            return;
        }
        updateBuilds();
        const intervalId = setInterval(updateBuilds, 10000);
        return () => clearInterval(intervalId);
    }, [app]);


    if (app.sourceType === 'container') {
        return <></>;
    }

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.deployments.title')}</CardTitle>
                <CardDescription>{t('app.deployments.description')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                {error ? <p className="py-4 text-sm text-destructive">{error}</p> : !appBuilds ? <FullLoadingSpinner /> :
                    <SimpleDataTable columns={[
                        ['replicasetName', t('app.deployments.deploymentName'), false],
                        ['buildJobName', t('app.deployments.buildJobName'), false],
                        ['deploymentId', t('app.deployments.deploymentId'), false],
                        ['status', t('common.status'), true, (item) => <DeploymentStatusBadge>{item.status}</DeploymentStatusBadge>],
                        ["startTime", t('app.deployments.startedAt'), true, (item) => formatDateTime(item.createdAt)],
                        ['gitCommit', 'Git Commit', true, (item) => <ShortCommitHash>{item.gitCommit}</ShortCommitHash>],
                    ]}
                        data={appBuilds}
                        hideSearchBar={true}
                        actionCol={(item) => {
                            return <>
                                <div className="flex gap-4">
                                    <div className="flex-1"></div>
                                    {item.deploymentId && <Button variant="secondary" onClick={() => setSelectedDeploymentForLogs(item)}>{t('app.deployments.showLogs')}</Button>}
                                    {role === RolePermissionEnum.READWRITE && item.buildJobName && item.status === 'BUILDING' && <Button variant="destructive" onClick={() => deleteBuildClick(item.buildJobName!)}>{t('app.deployments.stopBuild')}</Button>}
                                </div>
                            </>
                        }}
                    />
                }
            </CardContent>
        </Card>
        <BuildLogsDialog deploymentInfo={selectedDeploymentForLogs} onClose={() => setSelectedDeploymentForLogs(undefined)} />
    </>;
}
