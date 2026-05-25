import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useEffect, useState } from "react";
import { createNewWebhookUrl } from "./actions";
import { Button } from "@/components/ui/button";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Toast } from "@/frontend/utils/toast.utils";
import { ClipboardCopy } from "lucide-react";
import { toast } from "sonner";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";

export default function WebhookDeploymentInfo({
    app,
    role
}: {
    app: any;
    role: RolePermissionEnum;
}) {
    const { openConfirmDialog } = useConfirmDialog();
    const [webhookUrl, setWebhookUrl] = useState<string | undefined>(undefined);

    useEffect(() => {
        // Rust backend handles webhooks directly at /webhooks/:webhook_id
        const webhookId = app.webhook_id ?? app.webhookId;
        if (webhookId) {
            const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL ?? '';
            setWebhookUrl(`${backendUrl}/webhooks/${webhookId}`);
        }
    }, [app]);

    const createNewWebhookUrlAsync = async () => {
        if (!await openConfirmDialog({
            title: 'Generate new Webhook URL',
            description: 'Are you sure you want to generate a new Webhook URL? The old URL will be invalidated.',
            okButton: 'Generate new URL'
        })) {
            return;
        }
        await Toast.fromAction(() => createNewWebhookUrl(app.project_id ?? app.projectId, app.id), 'Webhook URL has been regenerated.');
    }

    const copyWebhookUrl = () => {
        navigator.clipboard.writeText(webhookUrl!);
        toast.success('Webhook URL has been copied to clipboard.');
    }

    return <>
        <Card>
            <CardHeader>
                <CardTitle>Webhook Deployment</CardTitle>
                <CardDescription>Use this webhook URL to trigger deployments from external services (for example GitHub Actions or GitLab Pipelines).</CardDescription>
            </CardHeader>
            <CardContent>
                <div className="flex gap-4">
                    {webhookUrl && <Button className="flex-1 truncate" variant="secondary" onClick={copyWebhookUrl}>
                        <span className="truncate">{webhookUrl}</span> <ClipboardCopy />
                    </Button>}
                    {role === RolePermissionEnum.READWRITE && <Button onClick={createNewWebhookUrlAsync} variant={webhookUrl ? 'ghost' : 'secondary'}>{webhookUrl ? 'Generate new Webhook URL' : 'Enable Webhook deployments'}</Button>}
                </div>
            </CardContent>
        </Card>
    </>;
}
