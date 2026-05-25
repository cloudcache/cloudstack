'use client'

import { InputDialog } from "@/components/custom/input-dialog"
import { Button } from "@/components/ui/button"
import { Toast } from "@/frontend/utils/toast.utils";
import { createProject } from "./actions";
import { useInputDialog } from "@/frontend/states/zustand.states";
import { Project } from "@/shared/model/prisma-compat";


export function EditProjectDialog({ children, existingItem }: { children?: React.ReactNode, existingItem?: Project }) {

    const { openInputDialog } = useInputDialog();
    const createProj = async () => {
        const name = await openInputDialog({
            title: "Create Project",
            description: "Name your new project.",
            fieldName: "Name",
            okButton: "Create Project",
            inputValue: existingItem?.name ?? ''
        })
        if (!name) { return; }
        await Toast.fromAction(() => createProject(name, existingItem?.id));
    };

    return <div onClick={() => createProj()}>{children}</div>
}