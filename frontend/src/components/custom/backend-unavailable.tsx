import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertTriangle } from "lucide-react";

export function BackendUnavailable({
    title = "Backend service unavailable",
    message = "The backend service cannot be reached right now. Please check that it is running and try again.",
}: {
    title?: string;
    message?: string;
}) {
    return (
        <div className="flex-1 pt-6">
            <Alert variant="destructive">
                <AlertTriangle className="h-4 w-4" />
                <AlertTitle>{title}</AlertTitle>
                <AlertDescription>{message}</AlertDescription>
            </Alert>
        </div>
    );
}
