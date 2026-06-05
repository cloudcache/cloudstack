import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import DbGateDbTool from "./db-gate-db-tool";
import DbToolSwitch from "./phpmyadmin-db-tool";
import { useT } from "@/i18n";

export default function DbToolsCard({
    app
}: {
    app: AppExtendedModel;
}) {
    const t = useT();

    if (app.appType === 'REDIS') {
        return <></>;
    }

    return <>
        <Card>
            <CardHeader>
                <CardTitle>{t('app.dbTools.title')}</CardTitle>
                <CardDescription>{t('app.dbTools.description')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
                <DbGateDbTool app={app} />
                {['MYSQL', 'MARIADB'].includes(app.appType) && <DbToolSwitch app={app} toolId="phpmyadmin"
                    toolNameString="PHP My Admin" />}
                {app.appType === 'POSTGRES' && <DbToolSwitch app={app} toolId="pgadmin" toolNameString="pgAdmin" />}
            </CardContent>
        </Card >
    </>;
}
