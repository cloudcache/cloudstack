'use client'

import { ReactElement } from "react";
import { toast } from "sonner";
import { useT } from "@/i18n";

export function Code({ children, copieable = true, copieableValue, className }: { children: string | null | undefined, copieable?: boolean, copieableValue?: string, className?: string }) {
    const t = useT();
    return (children &&
        <code className={(className ?? '') + ' relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold ' + (copieable ? 'cursor-pointer' : '')}
            onClick={() => {
                if (!copieable) return;
                navigator.clipboard.writeText(copieableValue || children || '');
                toast.success(t('common.copiedToClipboard'));
            }}>
            {children}
        </code>
    )
}
