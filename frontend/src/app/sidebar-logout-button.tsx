'use client'

import { signOut } from "next-auth/react";
import { DropdownMenuItem } from "@/components/ui/dropdown-menu"
import { LogOut } from "lucide-react"
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { useT } from "@/i18n";

export function SidebarLogoutButton() {

    const { openConfirmDialog: openDialog } = useConfirmDialog();
    const t = useT();

    const signOutAsync = async () => {
        if (!await openDialog({
            title: t("auth.signOut"),
            description: t("auth.signOutConfirm"),
            okButton: t("auth.signOut"),
        })) {
            return;
        }
        await signOut({
            callbackUrl: "/auth?signedOut=1",
            redirect: true
        });
    }
    return (
        <DropdownMenuItem onClick={() => signOutAsync()}>
            <LogOut />
            <span>{t("auth.signOut")}</span>
        </DropdownMenuItem>
    )
}
