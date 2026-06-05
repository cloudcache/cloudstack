'use client'

import { Button } from "@/components/ui/button";

import { EditAppDialog } from "./edit-app-dialog";
import { Blocks, Database, File, LayoutGrid, Plus } from "lucide-react";
import ChooseTemplateDialog from "./choose-template-dialog";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useState } from "react";
import { useT } from "@/i18n";


export default function CreateProjectActions({
    projectId,
}: {
    projectId: string;
}) {

    const t = useT();
    const [templateType, setTemplateType] = useState<"database" | "template" | undefined>(undefined);

    return (
        <>
            <ChooseTemplateDialog projectId={projectId} templateType={templateType} onClose={() => setTemplateType(undefined)} />
            <DropdownMenu>
                <DropdownMenuTrigger asChild><Button><Plus /> {t('page.project.createApp')}</Button></DropdownMenuTrigger>
                <DropdownMenuContent>
                    <EditAppDialog projectId={projectId}>
                        <DropdownMenuItem><File /> {t('page.project.emptyApp')}</DropdownMenuItem>
                    </EditAppDialog>
                    <DropdownMenuItem onClick={() => setTemplateType('database')}><Database /> {t('templates.database')}</DropdownMenuItem>
                    <DropdownMenuItem onClick={() => setTemplateType('template')}><Blocks /> {t('templates.template')}</DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </>
    )
}
