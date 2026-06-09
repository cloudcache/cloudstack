'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { deletePort, savePort } from "./actions";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { KubeObjectNameUtils } from "@/server/utils/kube-object-name.utils";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { QuestionMarkCircledIcon } from "@radix-ui/react-icons";
import { Code } from "@/components/custom/code";
import { ListUtils } from "@/shared/utils/list.utils";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import DefaultPortEditDialog from "./default-port-edit";
import { Button } from "@/components/ui/button";
import { EditIcon, Plus, TrashIcon } from "lucide-react";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { useT } from "@/i18n";

export default function InternalHostnames({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();

    const asyncDeleteDomain = async (portId: string) => {
        const confirm = await openDialog({
            title: t('app.ports.deleteTitle'),
            description: t('app.ports.deleteDescription'),
            okButton: t('app.ports.deleteButton')
        });
        if (confirm) {
            await Toast.fromAction(() => deletePort(app.projectId, app.id, portId));
        }
    };

    const internalUrl = KubeObjectNameUtils.toServiceName(app.id);

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.ports.title')}</CardTitle>
                <CardDescription>{t('app.ports.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('app.ports.count', { count: app.appPorts.length })}</TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('common.port')}</TableHead>
                            <TableHead className="w-[100px]">{t('common.action')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {app.appPorts.map(port => (
                            <TableRow key={port.id}>
                                <TableCell className="font-medium">
                                    {port.port}
                                </TableCell>
                                {!readonly && <TableCell className="font-medium  flex gap-2">
                                    <DefaultPortEditDialog appId={app.id} appPort={port}>
                                        <Button variant="ghost"><EditIcon /></Button>
                                    </DefaultPortEditDialog>
                                    <Button variant="ghost" onClick={() => asyncDeleteDomain(port.id)}>
                                        <TrashIcon />
                                    </Button>
                                </TableCell>}
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
            {!readonly && <CardFooter>
                <DefaultPortEditDialog appId={app.id}>
                    <Button><Plus /> {t('app.ports.add')}</Button>
                </DefaultPortEditDialog>
            </CardFooter>}
        </Card>

        <Card>
            <CardHeader>
                <CardTitle>{t('app.internalHostnames.title')}</CardTitle>
                <CardDescription>{t('app.internalHostnames.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                {ListUtils.removeDuplicates([
                    ...app.appPorts.map(p => p.port),
                    ...app.appDomains.map(d => d.port)
                ]).map(port => (
                    <div key={port} className="flex gap-1 pb-2">
                        <div><Code>{internalUrl + ':' + port}</Code></div>
                        <div className="self-center">
                            <TooltipProvider>
                                <Tooltip>
                                    <TooltipTrigger asChild><QuestionMarkCircledIcon /></TooltipTrigger>
                                    <TooltipContent>
                                        <p className="max-w-[350px]">
                                            {t('app.internalHostnames.tooltip')}<br /><br />
                                            <span className="font-bold">{t('app.domains.hostname')}:</span> {internalUrl}<br />
                                            <span className="font-bold">{t('common.port')}:</span> {port}
                                        </p>
                                    </TooltipContent>
                                </Tooltip>
                            </TooltipProvider>
                        </div>
                    </div>
                ))}
            </CardContent>
        </Card >
    </>;
}
