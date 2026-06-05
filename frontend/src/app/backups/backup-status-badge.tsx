'use client'

import { Tooltip, TooltipContent } from "@/components/ui/tooltip";
import { TooltipTrigger } from "@radix-ui/react-tooltip";
import { useT } from "@/i18n";

interface BackupStatusBadgeProps {
    missedBackup: boolean | undefined;
}

export default function BackupStatusBadge({ missedBackup }: BackupStatusBadgeProps) {
    const t = useT();
    if (missedBackup === undefined) {
        return null;
    }

    if (missedBackup) {
        return (
            <Tooltip>
                <TooltipTrigger>
                    <span className="px-2 py-1 rounded-lg text-sm font-semibold bg-orange-100 text-orange-800">
                        {t('common.warning')}
                    </span>
                </TooltipTrigger>
                <TooltipContent>
                    <p className="max-w-60">{t('page.backups.missedWarning')}</p>
                </TooltipContent>
            </Tooltip>
        );
    }

    return (
        <span className="px-2 py-1 rounded-lg text-sm font-semibold bg-green-100 text-green-800">
            OK
        </span>
    );
}
