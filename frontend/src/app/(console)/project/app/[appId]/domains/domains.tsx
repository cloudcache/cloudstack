'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { CheckIcon, EditIcon, Plus, TrashIcon, XIcon } from "lucide-react";
import DialogEditDialog from "./domain-edit-overlay";
import { Toast } from "@/frontend/utils/toast.utils";
import { deleteDomain } from "./actions";
import { Code } from "@/components/custom/code";
import { OpenInNewWindowIcon } from "@radix-ui/react-icons";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { useT } from "@/i18n";


export default function DomainsList({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {

    const t = useT();
    const { openConfirmDialog: openDialog } = useConfirmDialog();

    const asyncDeleteDomain = async (domainId: string) => {
        const confirm = await openDialog({
            title: t('app.domains.deleteTitle'),
            description: t('app.domains.deleteDescription'),
            okButton: t('app.domains.deleteButton')
        });
        if (confirm) {
            await Toast.fromAction(() => deleteDomain(app.projectId, app.id, domainId));
        }
    };

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.domains.title')}</CardTitle>
                <CardDescription>{t('app.domains.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <Table>
                    <TableCaption>{t('app.domains.count', { count: app.appDomains.length })}</TableCaption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t('common.name')}</TableHead>
                            <TableHead>{t('common.port')}</TableHead>
                            <TableHead>SSL</TableHead>
                            <TableHead>{t('app.domains.redirectHttps')}</TableHead>
                            <TableHead className="w-[100px]">{t('common.action')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {app.appDomains.map(domain => (
                            <TableRow key={domain.hostname}>
                                <TableCell className="font-medium flex gap-2">
                                    <Code>{domain.hostname}</Code>
                                    <div className="self-center cursor-pointer" onClick={() => window.open((domain.useSsl ? 'https://' : 'http://') + domain.hostname, '_blank')}>
                                        <OpenInNewWindowIcon />
                                    </div>
                                </TableCell>
                                <TableCell className="font-medium">{domain.port}</TableCell>
                                <TableCell className="font-medium">{domain.useSsl ? <CheckIcon /> : <XIcon />}</TableCell>
                                <TableCell className="font-medium">{domain.useSsl && domain.redirectHttps ? <CheckIcon /> : <XIcon />}</TableCell>
                                {!readonly && <TableCell className="font-medium flex gap-2">
                                    <DialogEditDialog appId={app.id} domain={domain}>
                                        <Button variant="ghost"><EditIcon /></Button>
                                    </DialogEditDialog>
                                    <Button variant="ghost" onClick={() => asyncDeleteDomain(domain.id)}>
                                        <TrashIcon />
                                    </Button>
                                </TableCell>}
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
            {!readonly && <CardFooter>
                <DialogEditDialog appId={app.id}>
                    <Button><Plus /> {t('app.domains.add')}</Button>
                </DialogEditDialog>
            </CardFooter>}
        </Card >

    </>;
}
