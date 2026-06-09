import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useEffect, useState } from "react";
import { createNewWebhookUrl } from "./actions";
import { Button } from "@/components/ui/button";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Toast } from "@/frontend/utils/toast.utils";
import { ClipboardCopy } from "lucide-react";
import { toast } from "sonner";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { useT } from "@/i18n";

export default function WebhookDeploymentInfo({
    app,
    role
}: {
    app: any;
    role: RolePermissionEnum;
}) {
    const t = useT();
    const { openConfirmDialog } = useConfirmDialog();
    const [webhookUrl, setWebhookUrl] = useState<string | undefined>(undefined);

    useEffect(() => {
        // Rust backend handles webhooks directly at /webhooks/:webhook_id
        const webhookId = app.webhook_id ?? app.webhookId;
        if (webhookId) {
            const backendUrl = (process.env.NEXT_PUBLIC_BACKEND_URL?.trim() || 'http://localhost:3001').replace(/\/+$/, '');
            setWebhookUrl(`${backendUrl}/webhooks/${webhookId}`);
        }
    }, [app]);

    const createNewWebhookUrlAsync = async () => {
        if (!await openConfirmDialog({
            title: t('app.webhook.generateTitle'),
            description: t('app.webhook.generateDescription'),
            okButton: t('app.webhook.generateButton')
        })) {
            return;
        }
        await Toast.fromAction(() => createNewWebhookUrl(app.project_id ?? app.projectId, app.id), t('app.webhook.regenerated'));
    }

    const copyWebhookUrl = () => {
        navigator.clipboard.writeText(webhookUrl!);
        toast.success(t('app.webhook.copied'));
    }

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.webhook.title')}</CardTitle>
                <CardDescription>{t('app.webhook.description')}</CardDescription>
            </CardHeader>
            <CardContent>
                <div className="flex gap-4">
                    {webhookUrl && <Button className="flex-1 truncate" variant="secondary" onClick={copyWebhookUrl}>
                        <span className="truncate">{webhookUrl}</span> <ClipboardCopy />
                    </Button>}
                    {role === RolePermissionEnum.READWRITE && <Button onClick={createNewWebhookUrlAsync} variant={webhookUrl ? 'ghost' : 'secondary'}>{webhookUrl ? t('app.webhook.generateTitle') : t('app.webhook.enable')}</Button>}
                </div>
            </CardContent>
        </Card>
    </>;
}
