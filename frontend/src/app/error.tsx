'use client'

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { AlertTriangle, RefreshCw } from "lucide-react";

export default function AppError({
    error,
    reset,
}: {
    error: Error & { digest?: string };
    reset: () => void;
}) {
    const t = useT();

    return (
        <div className="flex-1 pt-6">
            <Alert variant="destructive">
                <AlertTriangle className="h-4 w-4" />
                <AlertTitle>{t("error.pageLoadTitle")}</AlertTitle>
                <AlertDescription className="space-y-3">
                    <p>{error.message || t("error.unexpectedRetry")}</p>
                    <Button type="button" variant="outline" size="sm" onClick={reset}>
                        <RefreshCw className="h-4 w-4" />
                        {t("common.retry")}
                    </Button>
                </AlertDescription>
            </Alert>
        </div>
    );
}
