'use client'

import { Button } from "@/components/ui/button";

import Link from "next/link";
import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Edit2, Eye, MoreHorizontal, Trash } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { Project } from "@/shared/model/prisma-compat";
import { deleteProject } from "./actions";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { EditProjectDialog } from "./edit-project-dialog";
import { UserSession } from "@/shared/model/sim-session.model";
import { UserGroupUtils } from "@/shared/utils/role.utils";
import ProjectStatusIndicator from "@/components/custom/project-status-indicator";
import { useT } from "@/i18n";


export default function ProjectsTable({ data, session }: { data: Project[]; session: UserSession; }) {

    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const t = useT();

    const asyncDeleteProject = async (domainId: string) => {
        const confirm = await openDialog({
            title: t("page.projects.deleteTitle"),
            description: t("page.projects.deleteDescription"),
            okButton: t("page.projects.delete")
        });
        if (confirm) {
            await Toast.fromAction(() => deleteProject(domainId));
        }
    };

    return <>
        <SimpleDataTable columns={[
            ['id', t("common.id"), false],
            ['name', t("common.name"), true],
            ['status', t("common.status"), true, (item) => <ProjectStatusIndicator projectId={item.id} />],
            ["createdAt", t("common.createdAt"), true, (item) => formatDateTime(item.createdAt)],
            ["updatedAt", t("common.updatedAt"), false, (item) => formatDateTime(item.updatedAt)],
        ]}
            data={data}
            onItemClickLink={(item) => `/project/${item.id}`}
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
                                <Link href={`/project/${item.id}`}>
                                    <DropdownMenuItem>
                                        <Eye /> <span>{t("page.projects.showApps")}</span>
                                    </DropdownMenuItem>
                                </Link>
                                <DropdownMenuSeparator />
                                {UserGroupUtils.isAdmin(session) && <>
                                    <EditProjectDialog existingItem={item}>
                                        <DropdownMenuItem>
                                            <Edit2 /> <span>{t("page.projects.editName")}</span>
                                        </DropdownMenuItem>
                                    </EditProjectDialog>
                                    <DropdownMenuItem className="text-red-500" onClick={() => asyncDeleteProject(item.id)}>
                                        <Trash /> <span >{t("page.projects.delete")}</span>
                                    </DropdownMenuItem>
                                </>}
                            </DropdownMenuContent>
                        </DropdownMenu>
                    </div>
                </>}
        />
    </>
}
