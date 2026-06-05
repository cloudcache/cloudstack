'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { EditIcon, TrashIcon } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { deleteFileMount } from "./actions";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { AppVolume } from "@/shared/model/prisma-compat";
import React from "react";
import FileMountEditDialog from "./file-mount-edit-dialog";
import { useT } from "@/i18n";

type AppVolumeWithCapacity = (AppVolume & { capacity?: string });

export default function FileMount({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();

    const asyncDeleteFileMount = async (volumeId: string) => {
        const confirm = await openDialog({
            title: t('app.fileMount.deleteTitle'),
            description: t('app.fileMount.deleteDescription'),
            okButton: t('app.fileMount.deleteButton'),
        });
        if (confirm) {
            await Toast.fromAction(() => deleteFileMount(volumeId));
        }
    };

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.fileMount.title')}</CardTitle>
                <CardDescription>{t('app.fileMount.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('app.fileMount.count', { count: app.appFileMounts.length })}</TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('app.fileMount.mountPath')}</TableHead>
                            <TableHead className="w-[100px]">{t('common.action')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {app.appFileMounts.map(fileMount => (
                            <TableRow key={fileMount.containerMountPath}>
                                <TableCell className="font-medium">{fileMount.containerMountPath}</TableCell>
                                {!readonly && <TableCell className="font-medium flex gap-2">
                                    <FileMountEditDialog app={app} fileMount={fileMount}>
                                        <Button variant="ghost"><EditIcon /></Button>
                                    </FileMountEditDialog>
                                    <Button variant="ghost" onClick={() => asyncDeleteFileMount(fileMount.id)}>
                                        <TrashIcon />
                                    </Button>
                                </TableCell>}
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
            {!readonly && <CardFooter>
                <FileMountEditDialog app={app}>
                    <Button>{t('app.fileMount.add')}</Button>
                </FileMountEditDialog>
            </CardFooter>
            }
        </Card >
    </>;
}
