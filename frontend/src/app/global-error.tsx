'use client' // Error boundaries must be Client Components

import { Button } from "@/components/ui/button"
import { cn } from "@/frontend/utils/utils";
import { getMessages, normalizeLocale, translate } from "@/i18n/core";
import { AlertCircle } from "lucide-react"
import { Inter } from "next/font/google";

const inter = Inter({
    subsets: ["latin"],
    variable: "--font-sans",
});

export default function GlobalError({
    error,
    reset,
}: {
    error: Error & { digest?: string }
    reset: () => void
}) {
    const locale = typeof navigator === "undefined" ? "en" : normalizeLocale(navigator.language);
    const messages = getMessages(locale);
    const t = (key: string) => translate(messages, key);

    return (
        <html >
            <body className={cn(
                "min-h-screen bg-background font-sans antialiased",
                inter.variable
            )}>
                <div className="h-screen w-fuäll flex flex-col items-center justify-center p-4 space-y-4 bg-background text-foreground">
                    <div className="flex flex-col items-center justify-center space-y-2 text-center max-w-md">
                        <div className="rounded-full bg-destructive/10 p-3">
                            <AlertCircle className="h-8 w-8 text-destructive" />
                        </div>
                        <h2 className="text-2xl font-bold tracking-tight">{t("error.globalTitle")}</h2>
                        <p className="text-muted-foreground mt-4">
                            {t("error.globalDescription")}
                        </p>
                        <p className="text-xs text-muted-foreground mt-6">
                            {t("error.digest")}: {error.digest}
                        </p>
                    </div>
                </div>
            </body>
        </html>
    )
}
