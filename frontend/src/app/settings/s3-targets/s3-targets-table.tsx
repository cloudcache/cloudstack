'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Download, EditIcon, TrashIcon } from "lucide-react";
import DialogEditDialog from "./s3-target-edit-overlay";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { AppVolume, S3Target } from "@/shared/model/prisma-compat";
import React from "react";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { SimpleDataTable } from "@/components/custom/simple-data-table";
import { formatDateTime } from "@/frontend/utils/format.utils";
import S3TargetEditOverlay from "./s3-target-edit-overlay";
import { deleteVolume } from "@/app/project/app/[appId]/volumes/actions";
import { deleteS3Target } from "./actions";
import { useT } from "@/i18n";

export default function S3TargetsTable({ targets }: {
    targets: S3Target[]
}) {

    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const t = useT();

    const asyncDeleteTarget = async (id: string) => {
        const confirm = await openDialog({
            title: t("settings.s3Targets.deleteTitle"),
            description: t("settings.s3Targets.deleteDescription"),
            okButton: t("settings.s3Targets.deleteButton")
        });
        if (confirm) {
            await Toast.fromAction(() => deleteS3Target(id));
        }
    };

    return <>
        <SimpleDataTable columns={[
            ['id', t("common.id"), false],
            ['name', t("common.name"), true],
            ["createdAt", t("common.createdAt"), true, (item) => formatDateTime(item.createdAt)],
            ["updatedAt", t("common.updatedAt"), false, (item) => formatDateTime(item.updatedAt)],
        ]}
            data={targets}
            actionCol={(item) =>
                <>
                    <div className="flex">
                        <div className="flex-1"></div>
                        <DialogEditDialog target={item}>
                            <Button variant="ghost"><EditIcon /></Button>
                        </DialogEditDialog>
                        <Button variant="ghost" onClick={() => asyncDeleteTarget(item.id)}>
                            <TrashIcon />
                        </Button>
                    </div>
                </>}
        />
    </>;
}
