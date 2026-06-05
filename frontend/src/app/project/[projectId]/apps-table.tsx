'use client'

import { Button } from "@/components/ui/button";

import Link from "next/link";
import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Edit2, Eye, MoreHorizontal, Trash } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { App } from "@/shared/model/prisma-compat";
import { deleteApp } from "./actions";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { EditAppDialog } from "./edit-app-dialog";
import PodStatusIndicator from "@/components/custom/pod-status-indicator";
import { useT } from "@/i18n";


export default function AppTable({
    app,
    projectId,
    canCreateApps,
    canDeleteApps,
}: {
    app: App[],
    projectId: string,
    canCreateApps: boolean,
    canDeleteApps: boolean,
}) {

    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const t = useT();

    return <>
        <SimpleDataTable columns={[
            ['id', t("common.id"), false],
            ['name', t("common.name"), true],
            ['sourceType', t('app.table.sourceType'), false, (item) => item.sourceType === 'GIT' ? 'Git' : 'Container'],
            ['replicas', t('app.rateLimits.replicaCount'), false],
            ['memoryLimit', t('app.rateLimits.memoryLimitMb'), false],
            ['memoryReservation', t('app.rateLimits.memoryReservationMb'), false],
            ['cpuLimit', t('app.rateLimits.cpuLimitMillicores'), false],
            ['cpuReservation', t('app.rateLimits.cpuReservationMillicores'), false],
            ["createdAt", t("common.createdAt"), true, (item) => formatDateTime(item.createdAt)],
            ["updatedAt", t("common.updatedAt"), false, (item) => formatDateTime(item.updatedAt)],
            ['status', t("common.status"), true, (item) => <PodStatusIndicator appId={item.id} />],
        ]}
            data={app}
            onItemClickLink={(item) => `/project/app/${item.id}`}
            actionCol={(item) =>
                <>
                    <div className="flex">
                        <div className="flex-1"></div>
                        <DropdownMenu>
                            <DropdownMenuTrigger asChild>
                                <Button variant="ghost" className="h-8 w-8 p-0">
                                    <span className="sr-only">{t("dialog.openMenu")}</span>
                                    <MoreHorizontal className="h-4 w-4" />
                                </Button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                                <DropdownMenuLabel>{t("common.actions")}</DropdownMenuLabel>
                                <Link href={`/project/app/${item.id}`}>
                                    <DropdownMenuItem>
                                        <Eye /> <span>{t("page.project.showAppDetails")}</span>
                                    </DropdownMenuItem>
                                </Link>
                                <DropdownMenuSeparator />
                                {canCreateApps &&
                                    <EditAppDialog projectId={projectId} existingItem={item}>
                                        <DropdownMenuItem>
                                            <Edit2 /> <span>{t("page.project.editAppName")}</span>
                                        </DropdownMenuItem>
                                    </EditAppDialog>}
                {canDeleteApps && <DropdownMenuItem className="text-red-500"
                                    onClick={() => openDialog({
                                        title: t("page.project.deleteApp"),
                                        description: t("page.project.deleteAppDescription"),
                                    }).then((result) => result ? Toast.fromAction(() => deleteApp(projectId, item.id)) : undefined)}>
                                    <Trash />  <span >{t("page.project.deleteApp")}</span>
                                </DropdownMenuItem>}
                            </DropdownMenuContent>
                        </DropdownMenu>
                    </div>
                </>}
        />
    </>
}
