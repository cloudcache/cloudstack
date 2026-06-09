'use client';

import { Button } from "@/components/ui/button";
import { EditIcon, Plus, TrashIcon } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import React from "react";
import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { formatDateTime } from "@/frontend/utils/format.utils";
import { deleteRole } from "./actions";
import { adminRoleName } from "@/shared/model/role-extended.model.ts";
import RoleEditOverlay from "./user-group-edit-overlay";
import { ProjectExtendedModel } from "@/shared/model/project-extended.model";
import { UserGroupExtended } from "@/shared/model/sim-session.model";
import { useT } from "@/i18n";

export default function UserGroupsTable({ userGroups, projects }: {
    userGroups: UserGroupExtended[];
    projects: ProjectExtendedModel[];
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();

    const asyncDeleteItem = async (id: string) => {
        const confirm = await openDialog({
            title: t('users.groups.deleteTitle'),
            description: t('users.groups.deleteDescription'),
            okButton: t('common.delete'),
        });
        if (confirm) {
            await Toast.fromAction(() => deleteRole(id), t('users.groups.deleting'), t('users.groups.deleted'));
        }
    };

    return <>
        <SimpleDataTable columns={[
            ['id', t('common.id'), false],
            ['name', t('common.name'), true],
            ["createdAt", t('common.createdAt'), true, (item) => formatDateTime((item as any).createdAt)],
            ["updatedAt", t('common.updatedAt'), false, (item) => formatDateTime((item as any).updatedAt)],
        ]}
            data={userGroups}
            actionCol={(item) =>
                <>
                    <div className="flex">
                        {item.name !== adminRoleName && <>
                            <div className="flex-1"></div>
                            <RoleEditOverlay projects={projects} userGroup={item} >
                                <Button variant="ghost"><EditIcon /></Button>
                            </RoleEditOverlay>
                            <Button variant="ghost" onClick={() => asyncDeleteItem(item.id)}>
                                <TrashIcon />
                            </Button>
                        </>}
                    </div>
                </>}
        />
        <RoleEditOverlay projects={projects} >
            <Button variant="secondary"><Plus /> {t('users.groups.create')}</Button>
        </RoleEditOverlay>
    </>;
}
