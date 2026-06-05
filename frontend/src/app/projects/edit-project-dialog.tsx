'use client'

import { InputDialog } from "@/components/custom/input-dialog"
import { Button } from "@/components/ui/button"
import { Toast } from "@/frontend/utils/toast.utils";
import { createProject } from "./actions";
import { useInputDialog } from "@/frontend/states/zustand.states";
import { Project } from "@/shared/model/prisma-compat";
import { useT } from "@/i18n";


export function EditProjectDialog({ children, existingItem }: { children?: React.ReactNode, existingItem?: Project }) {

    const t = useT();
    const { openInputDialog } = useInputDialog();
    const createProj = async () => {
        const name = await openInputDialog({
            title: existingItem ? t('page.projects.editProject') : t('page.projects.createProject'),
            description: t('page.projects.nameProjectDescription'),
            fieldName: t('common.name'),
            okButton: existingItem ? t('common.save') : t('page.projects.createProject'),
            inputValue: existingItem?.name ?? ''
        })
        if (!name) { return; }
        await Toast.fromAction(() => createProject(name, existingItem?.id));
    };

    return <div onClick={() => createProj()}>{children}</div>
}
