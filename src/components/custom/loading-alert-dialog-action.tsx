'use client'

import { useState } from "react";
import { AlertDialogAction } from "../ui/alert-dialog";
import LoadingSpinner from "../ui/loading-spinner";

export default function LoadingAlertDialogAction({ onClick, children }: { onClick: () => Promise<void>, children: React.ReactNode }) {

    const [buttonIsLoading, setButtonIsLoading] = useState(false);
    return (
        <AlertDialogAction onClick={async () => {
            setButtonIsLoading(true);
            try {
                await onClick();
            } finally {
                setButtonIsLoading(false);
            }
        }} disabled={buttonIsLoading}>{buttonIsLoading ? <LoadingSpinner></LoadingSpinner> : children}</AlertDialogAction>
    )
}