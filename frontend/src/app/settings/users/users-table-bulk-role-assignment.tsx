'use client';

import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { UserExtended } from "@/shared/model/user-extended.model";
import { toast } from "sonner";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Toast } from "@/frontend/utils/toast.utils";
import { assignRoleToUsers } from "./actions";
import { UserGroupExtended } from "@/shared/model/sim-session.model";
import { useT } from "@/i18n";

interface UsersBulkRoleAssignmentProps {
    isOpen: boolean;
    onOpenChange: (open: boolean) => void;
    selectedUsers: UserExtended[];
    userGroups: UserGroupExtended[];
}

export default function UsersBulkRoleAssignment({
    isOpen,
    onOpenChange,
    selectedUsers,
    userGroups
}: UsersBulkRoleAssignmentProps) {
    const t = useT();
    const [selectedGroup, setSelectedGroup] = useState<string>("");

    const handleAssignGroup = async () => {
        if (!selectedGroup) {
            toast.error(t('users.selectGroupRequired'));
            return;
        }

        await Toast.fromAction(
            () => assignRoleToUsers(selectedUsers.map(u => u.id), selectedGroup),
            t('users.groupAssigned', { count: selectedUsers.length }),
            t('users.assigningGroup')
        );
        onOpenChange(false);
        setSelectedGroup("");
    };

    return (
        <Dialog open={isOpen} onOpenChange={onOpenChange}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>{t('users.assignGroup')}</DialogTitle>
                    <DialogDescription>
                        {t('users.assignGroupDescription', { count: selectedUsers.length })}
                    </DialogDescription>
                </DialogHeader>
                <Select onValueChange={setSelectedGroup} value={selectedGroup}>
                    <SelectTrigger>
                        <SelectValue placeholder={t('users.selectGroup')} />
                    </SelectTrigger>
                    <SelectContent>
                        {userGroups.map((role) => (
                            <SelectItem key={role.id} value={role.id}>
                                {role.name}
                            </SelectItem>
                        ))}
                    </SelectContent>
                </Select>
                <DialogFooter>
                    <Button variant="outline" onClick={() => onOpenChange(false)}>
                        {t('common.cancel')}
                    </Button>
                    <Button onClick={handleAssignGroup}>{t('users.assign')}</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
