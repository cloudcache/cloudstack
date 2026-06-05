'use client';

import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { useState } from "react";
import { Toast } from "@/frontend/utils/toast.utils";
import { saveNetworkPolicy } from "./actions";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { HelpCircle, AlertTriangle } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { useT } from "@/i18n";

export default function NetworkPolicy({ app, readonly }: {
    app: AppExtendedModel;
    readonly: boolean;
}) {
    const t = useT();
    const [ingressPolicy, setIngressPolicy] = useState(app.ingressNetworkPolicy);
    const [egressPolicy, setEgressPolicy] = useState(app.egressNetworkPolicy);
    const [useNetworkPolicy, setUseNetworkPolicy] = useState(app.useNetworkPolicy);
    const [showHelp, setShowHelp] = useState(false);

    const handleSave = async () => {
        await Toast.fromAction(() => saveNetworkPolicy(app.id, ingressPolicy, egressPolicy, useNetworkPolicy));
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('app.networkPolicy.title')}</CardTitle>
                <CardDescription>
                    {t('app.networkPolicy.description')}
                </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                <div className="flex items-center justify-between space-x-2 p-4 border rounded-lg">
                    <div className="space-y-0.5">
                        <Label htmlFor="use-network-policy">{t('app.networkPolicy.enable')}</Label>
                        <p className="text-sm text-muted-foreground">
                            {t('app.networkPolicy.enableDescription')}
                        </p>
                    </div>
                    <Switch
                        id="use-network-policy"
                        disabled={readonly}
                        checked={useNetworkPolicy}
                        onCheckedChange={setUseNetworkPolicy}
                    />
                </div>

                {!useNetworkPolicy && (
                    <Alert variant="destructive">
                        <AlertTriangle className="h-4 w-4" />
                        <AlertTitle>{t('common.warning')}</AlertTitle>
                        <AlertDescription>
                            {t('app.networkPolicy.disabledWarning')}
                        </AlertDescription>
                    </Alert>
                )}

                <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-2">
                        <Label htmlFor="ingress">{t('app.networkPolicy.ingress')}</Label>
                        <Select
                            disabled={readonly || !useNetworkPolicy}
                            value={ingressPolicy}
                            onValueChange={setIngressPolicy}
                        >
                            <SelectTrigger id="ingress">
                                <SelectValue placeholder={t('app.networkPolicy.selectIngress')} />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="ALLOW_ALL">{t('app.networkPolicy.allowAll')}</SelectItem>
                                <SelectItem value="INTERNET_ONLY">{t('app.networkPolicy.internetOnly')}</SelectItem>
                                <SelectItem value="NAMESPACE_ONLY">{t('app.networkPolicy.projectAppsOnly')}</SelectItem>
                                <SelectItem value="DENY_ALL">{t('app.networkPolicy.denyAll')}</SelectItem>
                            </SelectContent>
                        </Select>
                        <p className="text-sm text-muted-foreground">
                            {t('app.networkPolicy.ingressHelp')}
                        </p>
                    </div>
                    <div className="space-y-2">
                        <Label htmlFor="egress">{t('app.networkPolicy.egress')}</Label>
                        <Select
                            disabled={readonly || !useNetworkPolicy}
                            value={egressPolicy}
                            onValueChange={setEgressPolicy}
                        >
                            <SelectTrigger id="egress">
                                <SelectValue placeholder={t('app.networkPolicy.selectEgress')} />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="ALLOW_ALL">{t('app.networkPolicy.allowAll')}</SelectItem>
                                <SelectItem value="INTERNET_ONLY">{t('app.networkPolicy.internetOnly')}</SelectItem>
                                <SelectItem value="NAMESPACE_ONLY">{t('app.networkPolicy.projectAppsOnly')}</SelectItem>
                                <SelectItem value="DENY_ALL">{t('app.networkPolicy.denyAll')}</SelectItem>
                            </SelectContent>
                        </Select>
                        <p className="text-sm text-muted-foreground">
                            {t('app.networkPolicy.egressHelp')}
                        </p>
                    </div>
                </div>
            </CardContent>
            {!readonly && (
                <CardFooter className="gap-3">
                    <Button onClick={handleSave}>{t('common.save')}</Button>
                    <Dialog open={showHelp} onOpenChange={setShowHelp}>
                        <DialogTrigger asChild>
                            <Button variant="outline" size="icon">
                                <HelpCircle className="h-4 w-4" />
                            </Button>
                        </DialogTrigger>
                        <DialogContent className="max-w-2xl">
                            <DialogHeader>
                                <DialogTitle>{t('app.networkPolicy.typesTitle')}</DialogTitle>
                                <DialogDescription>
                                    {t('app.networkPolicy.typesDescription')}
                                </DialogDescription>
                            </DialogHeader>
                            <div className="space-y-4 py-4">
                                <div className="space-y-2">
                                    <h4 className="font-semibold text-sm">{t('app.networkPolicy.allowAll')}</h4>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.networkPolicy.allowAllDescription')}
                                    </p>
                                </div>
                                <div className="space-y-2">
                                    <h4 className="font-semibold text-sm">{t('app.networkPolicy.internetOnly')}</h4>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.networkPolicy.internetOnlyDescription')}
                                    </p>
                                </div>
                                <div className="space-y-2">
                                    <h4 className="font-semibold text-sm">{t('app.networkPolicy.projectAppsOnly')}</h4>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.networkPolicy.projectAppsOnlyDescription')}
                                    </p>
                                </div>
                                <div className="space-y-2">
                                    <h4 className="font-semibold text-sm">{t('app.networkPolicy.denyAll')}</h4>
                                    <p className="text-sm text-muted-foreground">
                                        {t('app.networkPolicy.denyAllDescription')}
                                    </p>
                                </div>
                            </div>
                        </DialogContent>
                    </Dialog>
                </CardFooter>
            )}
        </Card>
    );
}
